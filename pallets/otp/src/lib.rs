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

// use rand_chacha::{
//     ChaCha20Rng,
//     rand_core::SeedableRng,
// };

use etf_crypto_primitives::{
    ibe::fullident::BfIbe,
    client::etf_client::{AesIbeCt, DefaultEtfClient, EtfClient},
    utils::{convert_to_bytes, hash_to_g1},
};
// use ark_serialize::CanonicalDeserialize;

// use etf_crypto_primitives::{
// 	client::etf_client::{
// 		DefaultEtfClient, 
// 		EtfClient,
// 		DecryptionResult,
// 	},
// 	ibe::fullident::BfIbe,
// };

// use pallet_etf_aura::SlotSecretProvider;

pub type Name = BoundedVec<u8, ConstU32<32>>;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Encode, Decode, TypeInfo)]
pub struct OtpProxyDetails<AccountId> {
	pub address: AccountId,
	pub root: Vec<u8>,
}

// /// represents a timelock ciphertext
// #[derive(Debug, Clone, PartialEq, Decode, Encode, MaxEncodedLen, TypeInfo)]
// pub struct Ciphertext {
// 	/// the (AES) ciphertext
// 	pub ciphertext: BoundedVec<u8, ConstU32<512>>,
// 	/// the (AES) nonce
// 	pub nonce: BoundedVec<u8, ConstU32<96>>,
// 	/// the IBE ciphertext(s): for now we assume a single point in the future is used
// 	pub capsule: BoundedVec<u8, ConstU32<512>>,
// }

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
		// / Type representing a service that reads leaked slot secrets
		// type SlotSecretProvider: SlotSecretProvider;
	}

	/// a registry to track registered 'usernames' for OTP wallets
	/// Q: what happens when this map becomes very large? in terms of query time?
	#[pallet::storage]
	pub(super) type Registry<T: Config> =
		StorageMap<_, Blake2_256, Name, OtpProxyDetails<T::AccountId>, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		OTPProxyCreated
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		DuplicateName,
	}
 
	#[pallet::call]
	impl<T: Config> Pallet<T> {

		/// update the public parameters needed for the IBE scheme
		///
		/// * `g`: A generator of G1
		///
		#[pallet::weight(0)]
		#[pallet::call_index(0)]
		pub fn create(
			origin: OriginFor<T>,
			root: Vec<u8>, // should reall be T::Hash
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
			Registry::<T>::insert(name, &OtpProxyDetails { address, root });
			Self::deposit_event(Event::OTPProxyCreated);

			Ok(())
		}

		#[pallet::weight(0)]
		#[pallet::call_index(1)]
		pub fn proxy(
			origin: OriginFor<T>,
			name: BoundedVec<u8, ConstU32<32>>,
			otp: [u8;6], // 6 digit otp codes? a zkp could be better no?
			call: sp_std::boxed::Box<<T as Config>::RuntimeCall>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// Q: can we perform timelock decryption here or should it be offchain only?
			// let ct = DefaultEtfClient::<BfIbe>::decrypt(
			// 	ibe_params.1.clone(),
			// 	ibe_params.2.clone(),
			// 	&otp_code.as_bytes(), 
			// 	vec![id],
			// 	1,
			// 	&mut rng,
			// ).unwrap();

			Ok(())
		}
	}
}

// impl<T: Config> Pallet<T> {

	
// }
