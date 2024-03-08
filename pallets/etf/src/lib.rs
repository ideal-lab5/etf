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
	pallet_prelude::*,
	traits::ConstU32,
};
use sp_runtime::DispatchResult;
use ark_serialize::CanonicalDeserialize;

use etf_crypto_primitives::{
	client::etf_client::{
		AesIbeCt,
		DefaultEtfClient, 
		EtfClient,
		DecryptionResult,
	},
	ibe::fullident::BfIbe,
};

use pallet_etf_aura::SlotSecretProvider;

/// represents a timelock ciphertext
#[derive(Debug, Clone, PartialEq, Decode, Encode, MaxEncodedLen, TypeInfo)]
pub struct Ciphertext {
	/// the (AES) ciphertext
	pub ciphertext: BoundedVec<u8, ConstU32<512>>,
	/// the (AES) nonce
	pub nonce: BoundedVec<u8, ConstU32<96>>,
	/// the IBE ciphertext(s): for now we assume a single point in the future is used
	pub capsule: BoundedVec<u8, ConstU32<512>>,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use sp_runtime::DispatchResult;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_etf_aura::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// Type representing the weight of this pallet
		type WeightInfo: WeightInfo;
		/// Type representing a service that reads leaked slot secrets
		type SlotSecretProvider: SlotSecretProvider;
	}

	#[pallet::storage]
	#[pallet::getter(fn ibe_params)]
	pub(super) type IBEParams<T: Config> = StorageValue<
		_, (Vec<u8>, Vec<u8>, Vec<u8>), ValueQuery,
	>;

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
		G2DecodingFailure,
	}

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		// SCALE encoded?
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
	}
}

impl<T: Config> Pallet<T> {

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

/// errors for timelock encryption
pub enum TimelockError {
	DecryptionFailed,
	MissingSecret,
	BoundCallFailure,
	DecodeFailure,
}

/// provides timelock encryption using the current slot
pub trait TimelockEncryptionProvider {
	/// attempt to decrypt the ciphertext with the current slot secret
	fn decrypt_current(ciphertext: Ciphertext) -> Result<DecryptionResult, TimelockError>;
}

impl<T:Config> TimelockEncryptionProvider for Pallet<T> {

	fn decrypt_current(ciphertext: Ciphertext) -> Result<DecryptionResult, TimelockError> {
		if let Some(secret) = T::SlotSecretProvider::get() {
			let (_, p, _) = Self::ibe_params();
			let pt = DefaultEtfClient::<BfIbe>::decrypt(
				p, 
				ciphertext.ciphertext.to_vec(), 
				ciphertext.nonce.to_vec(), 
				vec![ciphertext.capsule.to_vec()], 
				vec![secret.to_vec()],
			).map_err(|_| TimelockError::DecryptionFailed)?;
			return Ok(pt);
		}
		Err(TimelockError::MissingSecret)
	}

}
