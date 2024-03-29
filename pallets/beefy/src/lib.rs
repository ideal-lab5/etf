// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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

use ark_std::Zero;
use ark_serialize::CanonicalDeserialize;
use etf_crypto_primitives::proofs::hashed_el_gamal_sigma::BatchPoK;

mod default_weights;
mod dpss;
mod equivocation;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use crate::equivocation::{EquivocationOffence, EquivocationReportSystem, TimeSlot};
pub use pallet::*;

use crate::equivocation::{EquivocationEvidenceFor};
pub use crate::dpss::CommitmentReportSystem;
use crate::dpss::CommitmentFor;

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

		/// The maximum number of nominators for each validator.
		#[pallet::constant]
		type MaxNominators: Get<u32>;

		/// The maximum number of entries to keep in the set id to session index mapping.
		///
		/// Since the `SetIdSession` map is only used for validating equivocations this
		/// value should relate to the bonding duration of whatever staking system is
		/// being used (if any). If equivocation handling is not enabled then this value
		/// can be zero.
		#[pallet::constant]
		type MaxSetIdSessionEntries: Get<u64>;

		/// A hook to act on the new BEEFY validator set.
		///
		/// For some applications it might be beneficial to make the BEEFY validator set available
		/// externally apart from having it in the storage. For instance you might cache a light
		/// weight MMR root over validators and make it available for Light Clients.
		type OnNewValidatorSet: OnNewValidatorSet<<Self as Config>::BeefyId>;

		/// Weights for this pallet.
		type WeightInfo: WeightInfo;

		/// The proof of key ownership, used for validating equivocation reports
		/// The proof must include the session index and validator count of the
		/// session at which the equivocation occurred.
		type KeyOwnerProof: Parameter + GetSessionNumber + GetValidatorCount;

		/// The equivocation handling subsystem.
		///
		/// Defines methods to publish, check and process an equivocation offence.
		type EquivocationReportSystem: OffenceReportSystem<
			Option<Self::AccountId>,
			EquivocationEvidenceFor<Self>,
		>;

		/// The commitment handling subsystem. 
		/// these aren't really 'offences' in the traditional sense
		/// because there really isn't anything 'wrong', we're really using this because
		/// the workflow it uses matches our use case
		///
		/// Defines methods to publish, check and process ACSS commitments
		type CommitmentReportSystem: OffenceReportSystem<
			Option<Self::AccountId>,
			CommitmentFor,
		>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// The current authorities set
	#[pallet::storage]
	pub type Authorities<T: Config> =
		StorageValue<_, BoundedVec<T::BeefyId, T::MaxAuthorities>, ValueQuery>;

	/// The current validator set id
	#[pallet::storage]
	pub type ValidatorSetId<T: Config> =
		StorageValue<_, sp_consensus_beefy::ValidatorSetId, ValueQuery>;

	/// Authorities set scheduled to be used with the next session
	#[pallet::storage]
	pub type NextAuthorities<T: Config> =
		StorageValue<_, BoundedVec<T::BeefyId, T::MaxAuthorities>, ValueQuery>;

	/// publicly verifiable shares for the current round (a resharing)
	/// here we assume that they follow the same order as the Authorities storage value vec
	/// later on we need to modify this to use merkle roots so we can change the ETF authority set
	#[pallet::storage]
	pub type Shares<T: Config> = 
		StorageValue<_, BoundedVec<BoundedVec<u8, ConstU32<1024>>, T::MaxAuthorities>, ValueQuery>;

	/// public commitments of the outputs of ACSS recovery for the current round
	#[pallet::storage]
	pub type Commitments<T: Config> = 
		StorageValue<_, BoundedVec<u8, T::MaxAuthorities>, ValueQuery>;

	/// A mapping from BEEFY set ID to the index of the *most recent* session for which its
	/// members were responsible.
	///
	/// This is only used for validating equivocation proofs. An equivocation proof must
	/// contains a key-ownership proof for a given session, therefore we need a way to tie
	/// together sessions and BEEFY set ids, i.e. we need to validate that a validator
	/// was the owner of a given key on a given session, and what the active set ID was
	/// during that session.
	///
	/// TWOX-NOTE: `ValidatorSetId` is not under user control.
	#[pallet::storage]
	pub type SetIdSession<T: Config> =
		StorageMap<_, Twox64Concat, sp_consensus_beefy::ValidatorSetId, SessionIndex>;

	/// Block number where BEEFY consensus is enabled/started.
	/// By changing this (through privileged `set_new_genesis()`), BEEFY consensus is effectively
	/// restarted from the newly set block number.
	#[pallet::storage]
	pub type GenesisBlock<T: Config> = StorageValue<_, Option<BlockNumberFor<T>>, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		/// Initial set of BEEFY authorities.
		pub authorities: Vec<T::BeefyId>,
		/// Block number where BEEFY consensus should start.
		/// Should match the session where initial authorities are active.
		/// *Note:* Ideally use block number where GRANDPA authorities are changed,
		/// to guarantee the client gets a finality notification for exactly this block.
		pub genesis_block: Option<BlockNumberFor<T>>,
		/// (beefy id, commitment, BatchPoK (which technically contains the commitment...))
		pub genesis_resharing: Vec<(T::BeefyId, Vec<u8>)>
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			// BEEFY genesis will be first BEEFY-MANDATORY block,
			// use block number one instead of chain-genesis.
			let genesis_block = Some(One::one());
			Self { authorities: Vec::new(), genesis_block, genesis_resharing: Vec::new() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			Pallet::<T>::initialize(&self.authorities)
				// we panic here as runtime maintainers can simply reconfigure genesis and restart
				// the chain easily
				.expect("Authorities vec too big");
			Pallet::<T>::initialize_genesis_shares(&self.genesis_resharing);
			GenesisBlock::<T>::put(&self.genesis_block);
		}
	}

	#[pallet::error]
	pub enum Error<T> {
		/// A key ownership proof provided as part of an equivocation report is invalid.
		InvalidKeyOwnershipProof,
		/// An equivocation proof provided as part of an equivocation report is invalid.
		InvalidEquivocationProof,
		/// A given equivocation report is valid but already previously reported.
		DuplicateOffenceReport,
		/// Submitted configuration is invalid.
		InvalidConfiguration,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Report voter equivocation/misbehavior. This method will verify the
		/// equivocation proof and validate the given key ownership proof
		/// against the extracted offender. If both are valid, the offence
		/// will be reported.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::report_equivocation(
			key_owner_proof.validator_count(),
			T::MaxNominators::get(),
		))]
		pub fn report_equivocation(
			origin: OriginFor<T>,
			equivocation_proof: Box<
				EquivocationProof<
					BlockNumberFor<T>,
					T::BeefyId,
					<T::BeefyId as RuntimeAppPublic>::Signature,
				>,
			>,
			key_owner_proof: T::KeyOwnerProof,
		) -> DispatchResultWithPostInfo {
			let reporter = ensure_signed(origin)?;

			T::EquivocationReportSystem::process_evidence(
				Some(reporter),
				(*equivocation_proof, key_owner_proof),
			)?;
			// Waive the fee since the report is valid and beneficial
			Ok(Pays::No.into())
		}

		/// Report voter equivocation/misbehavior. This method will verify the
		/// equivocation proof and validate the given key ownership proof
		/// against the extracted offender. If both are valid, the offence
		/// will be reported.
		///
		/// This extrinsic must be called unsigned and it is expected that only
		/// block authors will call it (validated in `ValidateUnsigned`), as such
		/// if the block author is defined it will be defined as the equivocation
		/// reporter.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::report_equivocation(
			key_owner_proof.validator_count(),
			T::MaxNominators::get(),
		))]
		pub fn report_equivocation_unsigned(
			origin: OriginFor<T>,
			equivocation_proof: Box<
				EquivocationProof<
					BlockNumberFor<T>,
					T::BeefyId,
					<T::BeefyId as RuntimeAppPublic>::Signature,
				>,
			>,
			key_owner_proof: T::KeyOwnerProof,
		) -> DispatchResultWithPostInfo {
			ensure_none(origin)?;

			T::EquivocationReportSystem::process_evidence(
				None,
				(*equivocation_proof, key_owner_proof),
			)?;
			Ok(Pays::No.into())
		}

		/// Reset BEEFY consensus by setting a new BEEFY genesis at `delay_in_blocks` blocks in the
		/// future.
		///
		/// Note: `delay_in_blocks` has to be at least 1.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::set_new_genesis())]
		pub fn set_new_genesis(
			origin: OriginFor<T>,
			delay_in_blocks: BlockNumberFor<T>,
		) -> DispatchResult {
			ensure_root(origin)?;
			ensure!(delay_in_blocks >= One::one(), Error::<T>::InvalidConfiguration);
			let genesis_block = frame_system::Pallet::<T>::block_number() + delay_in_blocks;
			GenesisBlock::<T>::put(Some(genesis_block));
			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(0)]
		pub fn report_commitment_unsigned(
			origin: OriginFor<T>,
			value: u8,
		) -> DispatchResult {
			ensure_none(origin)?;
			let _done = Commitments::<T>::try_mutate(
				|commitments| -> Result<(), DispatchError> {
					commitments.try_insert(commitments.len(), value)
						.expect("we trust non-authorities to behave right now");
				Ok(())
			});

			Ok(())
		}
	}

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		fn pre_dispatch(call: &Self::Call) -> Result<(), TransactionValidityError> {
			Self::pre_dispatch(call)
		}

		fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			Self::validate_unsigned(source, call)
		}
	}
}

use log::{error};
use frame_system::offchain::SubmitTransaction;

impl<T: Config> Pallet<T> {

	// /// try to read shares at a given index
	// pub fn read_share(at: u8) -> Option<Vec<u8>> {
	// 	// let shares = Shares::<T>::get();
	// 	// if at as usize >= shares.len() {
	// 	// 	return None;
	// 	// }
	// 	// Some(shares[at as usize].clone().into_inner())
	// }

	/// Return the current active BEEFY validator set.
	pub fn validator_set() -> Option<ValidatorSet<T::BeefyId>> {
		let validators: BoundedVec<T::BeefyId, T::MaxAuthorities> = Authorities::<T>::get();
		let id: sp_consensus_beefy::ValidatorSetId = ValidatorSetId::<T>::get();
		ValidatorSet::<T::BeefyId>::new(validators, id)
	}

	/// Submits an extrinsic to report a public commitment to the ACSS recovered shares
	/// This method will create an unsigned extrinsic with a call to `report_commitment_unsigned` and
	/// will push the transaction to the pool. Only useful in an offchain context.
	pub fn submit_unsigned_commitment(
		value: u8,
	) -> Option<()> {
		T::CommitmentReportSystem::publish_evidence(value).ok()
		// let call = Call::report_commitment_unsigned { value };
		// let res = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into());
		// match res {
		// 	Ok(_) => info!(target: LOG_TARGET, "Submitted commitment."),
		// 	Err(e) => error!(target: LOG_TARGET, "Error submitting commtiment: {:?}", e),
		// }
		// res
	}

	/// Submits an extrinsic to report an equivocation. This method will create
	/// an unsigned extrinsic with a call to `report_equivocation_unsigned` and
	/// will push the transaction to the pool. Only useful in an offchain context.
	pub fn submit_unsigned_equivocation_report(
		equivocation_proof: EquivocationProof<
			BlockNumberFor<T>,
			T::BeefyId,
			<T::BeefyId as RuntimeAppPublic>::Signature,
		>,
		key_owner_proof: T::KeyOwnerProof,
	) -> Option<()> {
		T::EquivocationReportSystem::publish_evidence((equivocation_proof, key_owner_proof)).ok()
	}

	fn change_authorities(
		new: BoundedVec<T::BeefyId, T::MaxAuthorities>,
		queued: BoundedVec<T::BeefyId, T::MaxAuthorities>,
	) {
		Authorities::<T>::put(&new);

		let new_id = ValidatorSetId::<T>::get() + 1u64;
		ValidatorSetId::<T>::put(new_id);

		NextAuthorities::<T>::put(&queued);

		if let Some(validator_set) = ValidatorSet::<T::BeefyId>::new(new, new_id) {
			let log = DigestItem::Consensus(
				BEEFY_ENGINE_ID,
				ConsensusLog::AuthoritiesChange(validator_set.clone()).encode(),
			);
			frame_system::Pallet::<T>::deposit_log(log);

			let next_id = new_id + 1;
			if let Some(next_validator_set) = ValidatorSet::<T::BeefyId>::new(queued, next_id) {
				<T::OnNewValidatorSet as OnNewValidatorSet<_>>::on_new_validator_set(
					&validator_set,
					&next_validator_set,
				);
			}
		}
	}

	fn initialize(authorities: &Vec<T::BeefyId>) -> Result<(), ()> {
		if authorities.is_empty() {
			return Ok(())
		}

		if !Authorities::<T>::get().is_empty() {
			return Err(())
		}

		let bounded_authorities =
			BoundedSlice::<T::BeefyId, T::MaxAuthorities>::try_from(authorities.as_slice())
				.map_err(|_| ())?;

		let id = GENESIS_AUTHORITY_SET_ID;
		Authorities::<T>::put(bounded_authorities);
		ValidatorSetId::<T>::put(id);
		// Like `pallet_session`, initialize the next validator set as well.
		NextAuthorities::<T>::put(bounded_authorities);

		if let Some(validator_set) = ValidatorSet::<T::BeefyId>::new(authorities.clone(), id) {
			let next_id = id + 1;
			if let Some(next_validator_set) =
				ValidatorSet::<T::BeefyId>::new(authorities.clone(), next_id)
			{
				<T::OnNewValidatorSet as OnNewValidatorSet<_>>::on_new_validator_set(
					&validator_set,
					&next_validator_set,
				);
			}
		}

		// NOTE: initialize first session of first set. this is necessary for
		// the genesis set and session since we only update the set -> session
		// mapping whenever a new session starts, i.e. through `on_new_session`.
		SetIdSession::<T>::insert(0, 0);

		Ok(())
	}

	fn initialize_genesis_shares(genesis_resharing: &Vec<(T::BeefyId, Vec<u8>)>) {
		// TODO: we need to convert back to BatchPoks and get the pubkey commitments
		// then we need to aggregate them and encode it onchain

		let mut round_pubkey = ark_bls12_377::G1Projective::zero();
		let mut unbounded_shares: Vec<BoundedVec<u8, ConstU32<1024>>> = Vec::new();
		// let mut shares:  = Vec::new();
		genesis_resharing.iter().for_each(|(public, pok_bytes)| {

			let bounded_pok =
				BoundedVec::<u8, ConstU32<1024>>::try_from(pok_bytes.clone())
					.expect("genesis poks should be well formatted");
			
			unbounded_shares.push(bounded_pok);

			// let pok: BatchPoK<ark_bls12_377::G1Projective> = 
			// 	BatchPoK::deserialize_compressed(&pok_bytes[..])
			// 		.expect("Genesis shares should be well formatted.");
			// we could also check the proofs here, but we can trust them on genesis for nows
		});
		
		let bounded_shares =
			BoundedVec::<BoundedVec<u8, ConstU32<1024>>, T::MaxAuthorities>::try_from(unbounded_shares)
				.expect("There should be the correct number of genesis resharings");
		<Shares<T>>::put(bounded_shares);
	}
}

impl<T: Config> sp_runtime::BoundToRuntimeAppPublic for Pallet<T> {
	type Public = T::BeefyId;
}

impl<T: Config> OneSessionHandler<T::AccountId> for Pallet<T>
where
	T: pallet_session::Config,
{
	type Key = T::BeefyId;

	fn on_genesis_session<'a, I: 'a>(validators: I)
	where
		I: Iterator<Item = (&'a T::AccountId, T::BeefyId)>,
	{
		let authorities = validators.map(|(_, k)| k).collect::<Vec<_>>();
		// we panic here as runtime maintainers can simply reconfigure genesis and restart the
		// chain easily
		Self::initialize(&authorities).expect("Authorities vec too big");
	}

	fn on_new_session<'a, I: 'a>(_changed: bool, validators: I, queued_validators: I)
	where
		I: Iterator<Item = (&'a T::AccountId, T::BeefyId)>,
	{
		let next_authorities = validators.map(|(_, k)| k).collect::<Vec<_>>();
		if next_authorities.len() as u32 > T::MaxAuthorities::get() {
			log::error!(
				target: LOG_TARGET,
				"authorities list {:?} truncated to length {}",
				next_authorities,
				T::MaxAuthorities::get(),
			);
		}
		let bounded_next_authorities =
			BoundedVec::<_, T::MaxAuthorities>::truncate_from(next_authorities);

		let next_queued_authorities = queued_validators.map(|(_, k)| k).collect::<Vec<_>>();
		if next_queued_authorities.len() as u32 > T::MaxAuthorities::get() {
			log::error!(
				target: LOG_TARGET,
				"queued authorities list {:?} truncated to length {}",
				next_queued_authorities,
				T::MaxAuthorities::get(),
			);
		}
		let bounded_next_queued_authorities =
			BoundedVec::<_, T::MaxAuthorities>::truncate_from(next_queued_authorities);

		// Always issue a change on each `session`, even if validator set hasn't changed.
		// We want to have at least one BEEFY mandatory block per session.
		Self::change_authorities(bounded_next_authorities, bounded_next_queued_authorities);

		let validator_set_id = ValidatorSetId::<T>::get();
		// Update the mapping for the new set id that corresponds to the latest session (i.e. now).
		let session_index = pallet_session::Pallet::<T>::current_index();
		SetIdSession::<T>::insert(validator_set_id, &session_index);
		// Prune old entry if limit reached.
		let max_set_id_session_entries = T::MaxSetIdSessionEntries::get().max(1);
		if validator_set_id >= max_set_id_session_entries {
			SetIdSession::<T>::remove(validator_set_id - max_set_id_session_entries);
		}
	}

	fn on_disabled(i: u32) {
		let log = DigestItem::Consensus(
			BEEFY_ENGINE_ID,
			ConsensusLog::<T::BeefyId>::OnDisabled(i as AuthorityIndex).encode(),
		);

		frame_system::Pallet::<T>::deposit_log(log);
	}
}

impl<T: Config> IsMember<T::BeefyId> for Pallet<T> {
	fn is_member(authority_id: &T::BeefyId) -> bool {
		Authorities::<T>::get().iter().any(|id| id == authority_id)
	}
}

pub trait WeightInfo {
	fn report_equivocation(validator_count: u32, max_nominators_per_validator: u32) -> Weight;
	fn set_new_genesis() -> Weight;
}
