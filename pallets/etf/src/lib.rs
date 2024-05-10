#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Encode, MaxEncodedLen};

use frame_support::{
	dispatch::{DispatchResultWithPostInfo, Pays},
	pallet_prelude::*,
	traits::{Get, OneSessionHandler},
	weights::Weight,
	BoundedSlice, BoundedVec, Parameter,
};
use frame_system::{
	ensure_none, ensure_signed,
	pallet_prelude::{BlockNumberFor, OriginFor},
};
use log;
use sp_runtime::{
	generic::DigestItem,
	traits::{IsMember, Member, One},
	RuntimeAppPublic,
};
use sp_session::{GetSessionNumber, GetValidatorCount};
use sp_staking::{offence::OffenceReportSystem, SessionIndex};
use sp_std::prelude::*;

use sp_consensus_beefy::{
	AuthorityIndex, BeefyAuthorityId, ConsensusLog, EquivocationProof, OnNewValidatorSet,
	ValidatorSet, BEEFY_ENGINE_ID, GENESIS_AUTHORITY_SET_ID,
};

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use pallet::*;

const LOG_TARGET: &str = "runtime::beefy";

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_system::{ensure_root, pallet_prelude::BlockNumberFor};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Authority identifier type
		type BeefyId: Member
			+ Parameter
			// todo: use custom signature hashing type instead of hardcoded `Keccak256`
			+ BeefyAuthorityId<sp_runtime::traits::Keccak256>
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		/// The maximum number of authorities that can be added.
		#[pallet::constant]
		type MaxAuthorities: Get<u32>;

		// TODO
		// /// Weights for this pallet.
		// type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// publicly verifiable shares for the current round (a resharing)
	#[pallet::storage]
	pub type Shares<T: Config> = 
		StorageValue<_, BoundedVec<BoundedVec<u8, ConstU32<1024>>, T::MaxAuthorities>, ValueQuery>;

	/// public commitments of the the expected validator to etf pubkey
	/// assumes order follows the same as the Authorities StorageValue 
	#[pallet::storage]
	pub type Commitments<T: Config> = 
		StorageValue<_, BoundedVec<T::BeefyId, T::MaxAuthorities>, ValueQuery>;

	/// the public key for the round (or rounds)
	#[pallet::storage]
	pub type RoundPublic<T: Config> = 
		StorageValue<_, BoundedVec<u8, ConstU32<144>>, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		/// (beefy id, commitment, BatchPoK (which technically contains the commitment...))
		pub genesis_resharing: Vec<(T::BeefyId, Vec<u8>)>,
		/// the round pubkey is the IBE master secret multiplied by a given group generator (e.g r = sP)
		pub round_pubkey: Vec<u8>,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { 
				genesis_resharing: Vec::new(),
				round_pubkey: Vec::new(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			Pallet::<T>::initialize(
				&self.genesis_resharing,
				self.round_pubkey.clone(),
			).expect("The genesis resharing should be correctly derived");
		}
	}

	#[pallet::error]
	pub enum Error<T> {
		TODO,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {

	}

}

impl<T: Config> Pallet<T> {

	fn initialize(
		genesis_resharing: &Vec<(T::BeefyId, Vec<u8>)>,
		round_key: Vec<u8>,
	) -> Result<(), ()>  {
		let bounded_rk =
			BoundedVec::<u8, ConstU32<144>>::try_from(round_key)
				.expect("The serialized round key should be 144 bytes.");
		<RoundPublic<T>>::put(bounded_rk);

		let mut unbounded_shares: Vec<BoundedVec<u8, ConstU32<1024>>> = Vec::new();
		
		genesis_resharing.iter().for_each(|(_commitment, pok_bytes)| {
			let bounded_pok =
				BoundedVec::<u8, ConstU32<1024>>::try_from(pok_bytes.clone())
					.expect("genesis poks should be well formatted");
			unbounded_shares.push(bounded_pok);
		});
		
		let bounded_shares =
			BoundedVec::<BoundedVec<u8, ConstU32<1024>>, T::MaxAuthorities>::try_from(
				unbounded_shares
			).expect("There should be the correct number of genesis resharings");
		<Shares<T>>::put(bounded_shares);

		let bounded_commitments =
			BoundedVec::<T::BeefyId, T::MaxAuthorities>::try_from(
				genesis_resharing.iter()
					.map(|g| g.0.clone())
					.collect::<Vec<_>>()
			).map_err(|_| ())?;

		Commitments::<T>::put(bounded_commitments);
		Ok(())
	}
}

/// A type to provide commitments, keys, and shares to validators
pub trait RoundCommitmentProvider<BeefyId, MaxAuthorities> {
	fn get() -> BoundedVec<BeefyId, MaxAuthorities>;
}

impl<T: Config> RoundCommitmentProvider<T::BeefyId, T::MaxAuthorities> for Pallet<T> {

	fn get() -> BoundedVec<T::BeefyId, T::MaxAuthorities> {
		Commitments::<T>::get()
	}
}
