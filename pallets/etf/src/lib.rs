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
pub mod weights;
pub use weights::WeightInfo;

use sp_std::{vec, vec::Vec, prelude::ToOwned};
use frame_support::{
	LOG_TARGET,
	pallet_prelude::*,
	BoundedSlice, BoundedVec,
};
use frame_system::{
	offchain::{
		CreateSignedTransaction, 
		Signer, 
		SendSignedTransaction,
		AppCrypto,
	},
	{self as system},
};
use sp_runtime::DispatchResult;
use ark_serialize::CanonicalDeserialize;
use sp_core::{crypto::KeyTypeId, ConstU32};

/// thye key type for the OCW
pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"etfn");
pub mod crypto {
	use super::KEY_TYPE;
	use sp_core::sr25519::Signature as Sr25519Signature;
	use sp_runtime::{
		app_crypto::{app_crypto, sr25519},
		traits::Verify, MultiSignature, MultiSigner
	};

	app_crypto!(sr25519, KEY_TYPE);

	pub struct ETFAuthorityId;

	// implemented for runtime
	impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for ETFAuthorityId {
		type RuntimeAppPublic = Public;
		type GenericSignature = sp_core::sr25519::Signature;
		type GenericPublic = sp_core::sr25519::Public;
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		pallet_prelude::*,
		traits::{CallerTrait, OriginTrait},
		dispatch::{GetDispatchInfo, PostDispatchInfo},
	};
	use sp_runtime::DispatchResult;
	use sp_runtime::traits::Dispatchable;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config + CreateSignedTransaction<Call<Self>> {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// Type representing the weight of this pallet
		type WeightInfo: WeightInfo;
		
		/// The maximum number of authorities that the pallet can hold.
		type MaxAuthorities: Get<u32>;

		/// the identifier type for an OCW 
		type AuthorityId: AppCrypto<Self::Public, Self::Signature>;
		// /// The identifier type for an authority.
		// type AuthorityId: Member
		// 	+ Parameter
		// 	+ RuntimeAppPublic
		// 	+ MaybeSerializeDeserialize
		// 	+ MaxEncodedLen;


		/// the identifier for an authority's paillier encryption key
		type PEK: Member
			+ Default
			+ Parameter
			+ MaybeSerializeDeserialize;
		// /// Type representing a service that reads leaked slot secrets
		// type SlotSecretProvider: SlotSecretProvider;
	}

	// // serialized capsules (TODO)
	// #[pallet::storage]
	// #[pallet::getter(fn shares)]
	// pub(super) type Shares<T: Config> = StorageMap<
	// 	_, 
	// 	Twox64Concat, 
	// 	EK, 
	// 	Vec<u8>, 
	// 	OptionQuery,
	// >;

	// /// the (serialized) ACSS Params, generators (G, H) of bls12-381
	// #[pallet::storage]
	// #[pallet::getter(fn acss_params)]
	// pub(super) type ACSSParams<T: Config> = StorageValue<_, Vec<u8>, ValueQuery>;

	
	/// The current offchain worker authority set
	#[pallet::storage]
	#[pallet::getter(fn authorities)]
	pub(super) type Authorities<T: Config> =
		StorageValue<_, BoundedVec<(T::Public, T::PEK), T::MaxAuthorities>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn ibe_params)]
	pub(super) type IBEParams<T: Config> = StorageValue<
		_, (Vec<u8>, Vec<u8>, Vec<u8>), ValueQuery,
	>;

	#[pallet::storage]
	pub(super) type Test<T: Config> = StorageValue<_, u8, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// the public ibe params were updated successfully
		IBEParamsUpdated,
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// the vector could not be decoded to an element of G1
		G1DecodingFailure,
		/// the vector could not be decoded to an element of G2
		G2DecodingFailure,
	}

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		// TODO: I want to pass the keys for each of the OCWs and map them to the paillier keys
		// the problem: T::Authorities is not a seriazable type
		// so I need to somehow get the public key type, which is serializable
		// pub authorities: Vec<<T as pallet::Config>::AuthorityId as sp_runtime::sr25519::Public>,
		// pub authorities: Vec<T::PEK>,
		// I guess this works for now but it feels incorrect
		pub authorities: Vec<(sp_core::sr25519::Public, T::PEK)>,
		pub initial_ibe_params: Vec<u8>,
		pub initial_ibe_pp: Vec<u8>,
		pub initial_ibe_commitment: Vec<u8>, 
		#[serde(skip)]
		pub _config: sp_std::marker::PhantomData<T>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			Pallet::<T>::set_ibe_params(
				&self.initial_ibe_params, &self.initial_ibe_pp, &self.initial_ibe_commitment)
				.expect("The input should be a valid generator of G1; qed");
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn offchain_worker(_: BlockNumberFor<T>) {
			log::info!("cool");
			Self::acss();
		}
	}

 
	#[pallet::call]
	impl<T: Config> Pallet<T> {

		/// update the public parameters needed for the IBE scheme
		///
		/// * `g`: A generator of G1
		///
		#[pallet::weight(<T as pallet::Config>::WeightInfo::update_ibe_params())]
		#[pallet::call_index(1)]
		pub fn update_ibe_params(
			origin: OriginFor<T>,
			g: Vec<u8>,
			ibe_pp_bytes: Vec<u8>,
			ibe_commitment_bytes: Vec<u8>,
		) -> DispatchResult {
			ensure_root(origin)?;
			Self::set_ibe_params(&g, &ibe_pp_bytes, &ibe_commitment_bytes)?;
			Self::deposit_event(Event::IBEParamsUpdated);
			Ok(())
		}

		#[pallet::weight(<T as pallet::Config>::WeightInfo::update_ibe_params())]
		#[pallet::call_index(2)]
		pub fn submit_acss_public_key(
			origin: OriginFor<T>,
			// bytes: [u8;32],
		) -> DispatchResult {
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {

	pub fn acss() {
		if sp_io::offchain::is_validator() {
			let signer = Signer::<T, T::AuthorityId>::all_accounts();

			if !signer.can_sign() {
				log::warn!(
					target: LOG_TARGET,
					"Skipping offchain worker because no local account is available."
				);
				return;
			}

			signer.send_signed_transaction(|acct| {
				let public = &acct.public;
				let eks = <Authorities<T>>::get()
					.into_iter()
					.filter(|auth| auth.0.eq(&public))
					.collect::<Vec<_>>();
				if !eks.is_empty() {
					let ek = &eks[0];
					log::info!("****************** FOUND EK {:?}", ek);
					panic!("");
				}
				log::info!("****************** DID NOT FIND EK");
				panic!("");
			});

			// then use the signer to get the ek
			// then use the ek to get the share (assume 1 per validator on genesis for now)
			//  run the ACSS::Restore algorithm
			// and send a signed transaction containing the commitment

			// let ek_ref = StorageValueRef::persistent(b"etf::ek");
			// if let Ok(Some(ek)) = ek_ref.get::<EK>() {
			// 	// find your capsule 
			// 	if let Some(share) = Shares::<T>::get(ek) {
			// 	// run acss::authenticate 
			// 	// store in offchain storage	
			// 	}	
			// }
		}
		// find your public key (from offchain storage)? i don't really love that
	}

	// /// Initial authorities.
	// ///
	// /// The storage will be applied immediately.
	// ///
	// /// The authorities length must be equal or less than T::MaxAuthorities.
	// pub fn initialize_authorities_paillier(authorities: Vec<(T::AuthorityId, T::PEK)>) {
	// 	if !authorities.is_empty() {
	// 		// assert!(<Authorities<T>>::get().is_empty(), "Authorities are already initialized!");
	// 		// assert!(<PaillierAuthorities<T>>::keys().len() > 0, "Paillier Authority keys are already initialized!");
	// 		authorities.iter().for_each(|a| {
	// 			// TODO: check that authorities have not been initialized
	// 			// if 
	// 			// let bounded = <BoundedSlice<'_, _, T::MaxAuthorities>>::try_from(a.1)
	// 			// 	.expect("Initial authority set must be less than T::MaxAuthorities");
	// 			<PaillierAuthorities<T>>::insert(a.0.clone(), &a.1);
	// 		});
	// 	}
	// }

	// /// Initial shares
	// ///
	// /// The storage will be applied immediately.
	// ///
	// /// The shares length must be equal or less than T::MaxAuthorities.
	// pub fn initialize_shares(shares: &Vec<(EK, Vec<u8>)>) {
	// 	if !shares.is_empty() {
	// 		// assert!(<Shares<T>>::keys().is_empty(), "Shares are already initialized!");
	// 		// let bounded = <BoundedSlice<'_, _, <T as pallet_etf_aura::Config>::MaxAuthorities>>::try_from(shares)
	// 		// 	.expect("Initial shares amount must be less than T::MaxAuthorities");
	// 		shares.iter().for_each(|(acct, bytes)| {
	// 			<Shares<T>>::insert(acct, &bytes);
	// 		});
	// 	}
	// }

	// /// Initial the ACSS parameters
	// ///
	// /// The storage will be applied immediately.
	// ///
	// pub fn initialize_acss(bytes: &Vec<u8>) {
	// 	if !bytes.is_empty() {
	// 		assert!(<ACSSParams<T>>::get().is_empty(), "ACSS params are already initialized!");
	// 		// let bounded = <BoundedSlice<'_, _, <T as pallet_etf_aura::Config>::MaxAuthorities>>::try_from(shares)
	// 		// 	.expect("Initial shares amount must be less than T::MaxAuthorities");
	// 		<ACSSParams<T>>::put(bytes);
	// 	}
	// }

	/// attempt to deserialize the slice to an element of G1 
	/// and add it to storage if valid
	///
	/// `g`: A compressed and serialized element of G1
	///
	/// TODO: should also provide a DLEQ proof and verify it here
	pub fn set_ibe_params(g: &Vec<u8>, ibe_pp_bytes: &Vec<u8>, ibe_commitment_bytes: &Vec<u8>) -> DispatchResult {
		let _ = 
			ark_bls12_381::G1Affine::deserialize_compressed(&g[..])
			.map_err(|_| Error::<T>::G1DecodingFailure)?;
		let _ = 
			ark_bls12_381::G2Affine::deserialize_compressed(&ibe_pp_bytes[..])
			.map_err(|_| Error::<T>::G2DecodingFailure)?;
		let _ = 
			ark_bls12_381::G2Affine::deserialize_compressed(&ibe_commitment_bytes[..])
			.map_err(|_| Error::<T>::G2DecodingFailure)?;
		IBEParams::<T>::set((g.to_owned(), ibe_pp_bytes.to_owned(), ibe_commitment_bytes.to_owned()));
		Ok(())
	}
}

pub trait IBEParamProvider {
	fn get() -> (Vec<u8>, Vec<u8>, Vec<u8>);
}

impl<T: Config> IBEParamProvider for Pallet<T> {
	fn get() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
		Self::ibe_params()
	}
}