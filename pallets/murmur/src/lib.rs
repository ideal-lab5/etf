#![cfg_attr(not(feature = "std"), no_std)]

/// # EtF Pallet
///
/// The EtF pallet stores public parameters needed for the identity based encryption scheme
///
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
// pub mod weights;
// pub use weights::WeightInfo;

use codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_std::{vec, vec::Vec, prelude::ToOwned};
use frame_support::{
	pallet_prelude::*,
	traits::{ConstU32, IsSubType},
	dispatch::GetDispatchInfo,
};
use sp_runtime::{DispatchResult, traits::Dispatchable};
use ckb_merkle_mountain_range::{
	MerkleProof,
    MMR, Merge, Result as MMRResult,
    util::{MemMMR, MemStore},
};

use pallet_randomness_beacon::{Ciphertext, TimelockEncryptionProvider};

pub type Name = BoundedVec<u8, ConstU32<32>>;

#[derive(
	Debug, 
	PartialEq, Eq, Hash, 
	Clone, Encode, Decode, TypeInfo, 
	serde::Serialize, serde::Deserialize
)]
pub struct OtpProxyDetails<AccountId> {
	pub address: AccountId,
	pub root: Vec<u8>,
	pub size: u64,
}

// #[cfg(feature = "std")]
// use blake2b_rs::{Blake2b, Blake2bBuilder};
use sp_core::Bytes;
use sha3::Digest;

#[derive(Eq, PartialEq, Clone, Debug, Default)]
struct Leaf(pub Vec<u8>);
impl From<Vec<u8>> for Leaf {
    fn from(data: Vec<u8>) -> Self {
        // let mut hasher = new_blake2b();
        let mut hasher = sha3::Sha3_256::default();
        // let bytes = serde_json::to_vec(&data).unwrap();
        hasher.update(&data);
        let hash = hasher.finalize();
        Leaf(hash.to_vec().into())
        // NumberHash(num.into())
    }
}

struct MergeLeaves;

impl Merge for MergeLeaves {
    type Item = Leaf;
    fn merge(lhs: &Self::Item, rhs: &Self::Item) -> MMRResult<Self::Item> {
		let mut hasher = sha3::Sha3_256::default();
        hasher.update(&lhs.0);
        hasher.update(&rhs.0);
        let hash = hasher.finalize();
        Ok(Leaf(hash.to_vec().into()))
    }
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use sp_runtime::{
		DispatchResult,
		traits::Zero,
	};
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_proxy::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// The overarching call type.
		type RuntimeCall: Parameter
			+ Dispatchable<RuntimeOrigin = Self::RuntimeOrigin>
			+ GetDispatchInfo
			+ From<frame_system::Call<Self>>
			+ IsSubType<Call<Self>>
			+ IsType<<Self as frame_system::Config>::RuntimeCall>;
		// / Type representing the weight of this pallet
		// type WeightInfo: WeightInfo;
		/// something that can decrypt messages locked for the current slot
		type TlockProvider: TimelockEncryptionProvider<BlockNumberFor<Self>>;
	}

	/// a registry to track registered 'usernames' for OTP wallets
	/// Q: what happens when this map becomes very large? in terms of query time?
	#[pallet::storage]
	pub(super) type Registry<T: Config> =
		StorageMap<_, Blake2_256, Name, OtpProxyDetails<T::AccountId>, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		OtpProxyCreated,
		OtpProxyExecuted
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		BadCiphertext,
		DuplicateName,
		InvalidOTP,
		InvalidMerkleProof,
	}
 
	#[pallet::call]
	impl<T: Config> Pallet<T> {

		/// Vreate a time-based proxy account
		///
		/// * `root`: 
		///
		#[pallet::weight(0)]
		#[pallet::call_index(0)]
		pub fn create(
			origin: OriginFor<T>,
			root: Vec<u8>,
			size: u64,
			name: BoundedVec<u8, ConstU32<32>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// check duplicate name
			ensure!(Registry::<T>::get(name.clone()).is_none(), Error::<T>::DuplicateName);

			// create a pure proxy with no delegate
			let signed_origin: T::RuntimeOrigin = frame_system::RawOrigin::Signed(who.clone()).into();
			pallet_proxy::Pallet::<T>::create_pure(
				signed_origin,
				T::ProxyType::default(),
				BlockNumberFor::<T>::zero(),
				0u16,
				true,
			)?;

			let address = pallet_proxy::Pallet::<T>::pure_account(&who, &T::ProxyType::default(), 0, None);
			Registry::<T>::insert(name, &OtpProxyDetails { address, root, size });
			Self::deposit_event(Event::OtpProxyCreated);

			Ok(())
		}

		/// proxy a call after verifying the ciphertext
		/// this function first checks the validity of the merkle proof (using the ciphertext)
		/// if valid, it decrypts the ciphertext and uses it to verify the hash
		/// if valid, it proxies the call
		///
		#[pallet::weight(0)]
		#[pallet::call_index(1)]
		pub fn proxy(
			origin: OriginFor<T>,
			name: BoundedVec<u8, ConstU32<32>>,
			position: u64, // the position of the leaf in the mmr
			target_leaf: Vec<u8>, // TODO: the leaf of the mmr, should be bounded though
			hash: Vec<u8>, // a hash of the otp code and the call data
			proof: Vec<Vec<u8>>, // the merkle proof
			call: sp_std::boxed::Box<<T as pallet_proxy::Config>::RuntimeCall>,
			when: BlockNumberFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			if let Some(proxy_details) = Registry::<T>::get(name) {
				// verify the merkle proof
				let leaves: Vec<Leaf> = proof.clone().into_iter().map(|p| Leaf(p)).collect::<Vec<_>>();
				let size = proxy_details.size;
				let merkle_proof = MerkleProof::<Leaf, MergeLeaves>::new(size, leaves);
				let root = Leaf(proxy_details.root);

				ensure!(
					merkle_proof.verify(root, vec![(position, Leaf(target_leaf.clone()))]).unwrap(),
					Error::<T>::InvalidMerkleProof,
				);

				// now verify the hash 
			// we expect: hash = H(OTP || CALL_DATA)
				let result = T::TlockProvider::decrypt_at(&target_leaf, when)
					.map_err(|_| Error::<T>::BadCiphertext)?;
				let mut otp = result.message;
				let mut hasher = sha3::Sha3_256::default();
				hasher.update(otp);
				hasher.update(&call.encode());
				let h_b = hasher.finalize().to_vec();
				ensure!(h_b == hash, Error::<T>::InvalidOTP);

				let signed_origin: T::RuntimeOrigin = frame_system::RawOrigin::Signed(who.clone()).into();
				let def = pallet_proxy::Pallet::<T>::find_proxy(
					&proxy_details.address, 
					None, 
					Some(T::ProxyType::default())
				)?;
				// ensure!(def.delay.is_zero(), Error::<T>::Unannounced);
				pallet_proxy::Pallet::<T>::do_proxy(def, proxy_details.address, *call);
				Self::deposit_event(Event::OtpProxyExecuted);
			} else {
				// an error
			}

			Ok(())
		}
	}
}

// impl<T: Config> Pallet<T> {

	
// }
