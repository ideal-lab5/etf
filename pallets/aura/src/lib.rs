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

//! # Aura Module
//!
//! - [`Config`]
//! - [`Pallet`]
//!
//! ## Overview
//!
//! The Aura module extends Aura consensus by managing offline reporting.
//!
//! ## Interface
//!
//! ### Public Functions
//!
//! - `slot_duration` - Determine the Aura slot-duration based on the Timestamp module
//!   configuration.
//!
//! ## Related Modules
//!
//! - [Timestamp](../pallet_timestamp/index.html): The Timestamp module is used in Aura to track
//! consensus rounds (via `slots`).

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	traits::{DisabledValidators, FindAuthor, Get, OnTimestampSet, OneSessionHandler, ValidatorSet, ValidatorSetWithIdentification},
	BoundedSlice, BoundedVec, ConsensusEngineId, Parameter,
};
use frame_system::{
	pallet_prelude::{BlockNumberFor, HeaderFor},
	{self as system},
};
// use log;
use sp_consensus_etf_aura::{
	AuthorityIndex, 
	ConsensusLog, 
	Slot, 
	digests::PreDigest, 
	AURA_ENGINE_ID, 
	OpaqueSecret,
};
use sp_runtime::{
	Perbill,
	generic::DigestItem,
	traits::{IsMember, Member, SaturatedConversion, Saturating, Zero, Convert},
	RuntimeAppPublic,
	offchain::storage::StorageValueRef,
};
use scale_info::TypeInfo;
use sp_core::{crypto::KeyTypeId, ConstU32};
use sp_std::prelude::*;

use pallet_etf::IBEParamProvider;
use etf_crypto_primitives::{
	client::etf_client::{
		DefaultEtfClient, 
		EtfClient,
		DecryptionResult,
	},
	ibe::fullident::BfIbe,
	// dpss::acss::Capsule,
};

#[cfg(feature = "std")]
use etf_crypto_primitives::dpss::acss::Capsule;

pub mod migrations;
mod mock;
mod tests;

pub use pallet::*;

/// the slot secret type 
pub type Secret = [u8;48];

/// Counter for the number of epochs that have passed.
pub type EpochIndex = u32;

const LOG_TARGET: &str = "runtime::aura";

/// A slot duration provider which infers the slot duration from the
/// [`pallet_timestamp::Config::MinimumPeriod`] by multiplying it by two, to ensure
/// that authors have the majority of their slot to author within.
///
/// This was the default behavior of the Aura pallet and may be used for
/// backwards compatibility.
///
/// Note that this type is likely not useful without the `experimental`
/// feature.
pub struct MinimumPeriodTimesTwo<T>(sp_std::marker::PhantomData<T>);

impl<T: pallet_timestamp::Config> Get<T::Moment> for MinimumPeriodTimesTwo<T> {
	fn get() -> T::Moment {
		<T as pallet_timestamp::Config>::MinimumPeriod::get().saturating_mul(2u32.into())
	}
}

/// An ETF-Aura consensus digest item with MMR root hash.
pub struct DepositEtfAuraDigest<T>(sp_std::marker::PhantomData<T>);

impl<T> pallet_mmr::primitives::OnNewRoot<sp_consensus_etf_aura::MmrRootHash> for DepositEtfAuraDigest<T>
where
	T: pallet_mmr::Config<Hashing = sp_consensus_etf_aura::MmrHashing>,
	T: crate::pallet::Config,
{
	fn on_new_root(root: &sp_consensus_etf_aura::MmrRootHash) {
		let digest = sp_runtime::generic::DigestItem::Consensus(
			sp_consensus_etf_aura::AURA_ENGINE_ID,
			codec::Encode::encode(&sp_consensus_etf_aura::ConsensusLog::<
				T::AuthorityId,
			>::MmrRoot(*root)),
		);
		<frame_system::Pallet<T>>::deposit_log(digest);
	}
}

type MerkleRootOf<T> = <<T as pallet_mmr::Config>::Hashing as sp_runtime::traits::Hash>::Output;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		pallet_prelude::*,
		traits::{CallerTrait, OriginTrait},
		dispatch::{GetDispatchInfo, PostDispatchInfo},
	};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::Dispatchable;

	#[pallet::config]
	pub trait Config: frame_system::Config 
		+ pallet_timestamp::Config
		+ pallet_mmr::Config {
		
		/// The identifier type for an authority.
		type AuthorityId: Member
			+ Parameter
			+ RuntimeAppPublic
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		/// something that provides parameters for identity based encryption
		type IBEParamProvider: pallet_etf::IBEParamProvider;

		/// The maximum number of authorities that the pallet can hold.
		type MaxAuthorities: Get<u32>;

		/// A way to check whether a given validator is disabled and should not be authoring blocks.
		/// Blocks authored by a disabled validator will lead to a panic as part of this module's
		/// initialization.
		type DisabledValidators: DisabledValidators;

		/// Whether to allow block authors to create multiple blocks per slot.
		///
		/// If this is `true`, the pallet will allow slots to stay the same across sequential
		/// blocks. If this is `false`, the pallet will require that subsequent blocks always have
		/// higher slots than previous ones.
		///
		/// Regardless of the setting of this storage value, the pallet will always enforce the
		/// invariant that slots don't move backwards as the chain progresses.
		///
		/// The typical value for this should be 'false' unless this pallet is being augmented by
		/// another pallet which enforces some limitation on the number of blocks authors can create
		/// using the same slot.
		type AllowMultipleBlocksPerSlot: Get<bool>;

		/// the paillier encryption key type
		type PEK: Member
			+ Default
			+ Parameter
			+ MaybeSerializeDeserialize
			+ AsRef<[u8]>;

		/// Current leaf version.
		///
		/// Specifies the version number added to every leaf that get's appended to the MMR.
		/// Read more in [`MmrLeafVersion`] docs about versioning leaves.
		type LeafVersion: Get<MmrLeafVersion>;

		// / the threshold of honest participants required per round
		// type ThresholdPercent: Get<Perbill>;

		/// The slot duration Aura should run with, expressed in milliseconds.
		/// The effective value of this type should not change while the chain is running.
		///
		/// For backwards compatibility either use [`MinimumPeriodTimesTwo`] or a const.
		///
		/// This associated type is only present when compiled with the `experimental`
		/// feature.
		#[cfg(feature = "experimental")]
		type SlotDuration: Get<<Self as pallet_timestamp::Config>::Moment>;
	}

	#[pallet::pallet]
	#[pallet::without_storage_info] // TODO
	pub struct Pallet<T>(sp_std::marker::PhantomData<T>);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {

		#[cfg(feature = "std")]
		fn on_initialize(_: BlockNumberFor<T>) -> Weight {
			if let Some(predigest) = Self::current_predigest_from_digests() {
				let new_secret = predigest.secret;
				let new_slot = predigest.slot;

				let current_slot = CurrentSlot::<T>::get();
				
				if T::AllowMultipleBlocksPerSlot::get() {
					assert!(current_slot <= new_slot, "Slot must not decrease");
				} else {
					assert!(current_slot < new_slot, "Slot must increase");
				}

				Self::add_slot_secret(new_slot, new_secret);
				Self::set_current_slot(new_slot);

				if let Some(n_authorities) = <Authorities<T>>::decode_len() {
					let authority_index = *new_slot % n_authorities as u64;
					if T::DisabledValidators::is_disabled(authority_index as u32) {
						panic!(
							"Validator with index {:?} is disabled and should not be attempting to author blocks.",
							authority_index,
						);
					}
				}

				// TODO [#3398] Generate offence report for all authorities that skipped their
				// slots.

				T::DbWeight::get().reads_writes(2, 1)
			} else {
				T::DbWeight::get().reads(1)
			}
		}

		#[cfg(feature = "try-runtime")]
		fn try_state(_: BlockNumberFor<T>) -> Result<(), sp_runtime::TryRuntimeError> {
			Self::do_try_state()
		}
	}

	/// The current authority set.
	#[pallet::storage]
	#[pallet::getter(fn authorities)]
	pub(super) type Authorities<T: Config> =
		StorageValue<_, BoundedVec<T::AuthorityId, T::MaxAuthorities>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn next_authorities)]
	pub(super) type NextAuthorities<T: Config> =
		StorageValue<_, BoundedVec<(T::AuthorityId, T::PEK), T::MaxAuthorities>, ValueQuery>;

	/// The current registered authorities (who can participate in the key handoff protocol)
	#[pallet::storage]
	pub(super) type RegisteredAuthority<T: Config> =
		StorageMap<
			_, 
			Twox64Concat,
			T::AuthorityId,
			T::PEK,
			ValueQuery
		>;

	/// The current slot of this block.
	///
	/// This will be set in `on_initialize`.
	#[pallet::storage]
	#[pallet::getter(fn current_slot)]
	pub(super) type CurrentSlot<T: Config> = StorageValue<_, Slot, ValueQuery>;

	/// Current epoch index.
	#[pallet::storage]
	#[pallet::getter(fn epoch_index)]
	pub type CurrentEpoch<T> = StorageValue<_, EpochIndex, ValueQuery>;

	/// A map between slot numbers and slot secrets
	/// DLEQ proofs are stored in corresponding block headers
	#[pallet::storage]
	#[pallet::getter(fn slot_secrets)]
	pub(super) type SlotSecrets<T: Config> = StorageMap<
		_, 
		Twox64Concat, 
		Slot, 
		OpaqueSecret
	>;

	#[pallet::storage]
	#[pallet::getter(fn ibe_params)]
	pub(super) type IBEParams<T: Config> = StorageValue<
		_, (Vec<u8>, Vec<u8>, Vec<u8>), ValueQuery,
	>;

	/// the (serialized) ACSS Params, generators (G, H) of bls12-381
	#[pallet::storage]
	#[pallet::getter(fn acss_params)]
	pub(super) type ACSSParameters<T: Config> = StorageValue<_, Vec<u8>, ValueQuery>;
	

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		pub authorities: Vec<(T::AuthorityId, T::PEK)>,
		pub initial_shares: Vec<Capsule>,
		pub serialized_acss_params: Vec<u8>,
	}

	#[cfg(feature = "std")]
	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			Pallet::<T>::register_authorities(&self.authorities);
			Pallet::<T>::initialize_mmr(&self.initial_shares);
		}
	}

	
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		// TODO
		// report_equivocation?
	}

}

use pallet_mmr::{LeafDataProvider, ParentNumberAndHash};
use sp_consensus_etf_aura::mmr::{MmrLeaf, MmrLeafVersion};

impl<T: Config> LeafDataProvider for Pallet<T> {
	type LeafData = MmrLeaf<
		BlockNumberFor<T>,
		<T as frame_system::Config>::Hash,
		Vec<u8>,
	>;

	fn leaf_data() -> Self::LeafData {
		MmrLeaf {
			version: T::LeafVersion::get(),
			parent_number_and_hash: ParentNumberAndHash::<T>::leaf_data(),
			leaf_data: vec![],
			// T::ETFDataProvider::extra_data(),
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Change authorities.
	///
	/// The storage will be applied immediately.
	/// And aura consensus log will be appended to block's log.
	///
	/// This is a no-op if `new` is empty.
	pub fn change_authorities(new: BoundedVec<T::AuthorityId, T::MaxAuthorities>) {
		if new.is_empty() {
			log::warn!(target: LOG_TARGET, "Ignoring empty authority change.");

			return
		}

		<Authorities<T>>::put(&new);

		let log = DigestItem::Consensus(
			AURA_ENGINE_ID,
			ConsensusLog::AuthoritiesChange(new.into_inner()).encode(),
		);
		<frame_system::Pallet<T>>::deposit_log(log);
	}

	/// Initial authorities.
	///
	/// The storage will be applied immediately.
	///
	/// The authorities length must be equal or less than T::MaxAuthorities.
	pub fn initialize_authorities(authorities: &[T::AuthorityId]) {
		if !authorities.is_empty() {
			// assert!(<Authorities<T>>::get().is_empty(), "Authorities are already initialized!");
			let bounded = <BoundedSlice<'_, _, T::MaxAuthorities>>::try_from(authorities)
				.expect("Initial authority set must be less than T::MaxAuthorities");
			<Authorities<T>>::put(bounded);
		}
	}

	pub fn register_authorities(authorities: &[(T::AuthorityId, T::PEK)]) {
		if !authorities.is_empty() {
			authorities.iter().map(|a| {
				RegisteredAuthority::<T>::insert(a.0.clone(), &a.1.clone());
			});
			let bounded = <BoundedSlice<'_, _, T::MaxAuthorities>>::try_from(authorities)
				.expect("Initial authority set must be less than T::MaxAuthorities");
			NextAuthorities::<T>::put(bounded);
		}
	}

	#[cfg(feature = "std")]
	pub fn initialize_mmr(leaves: &[Capsule]) {
		let keyset_commitment = binary_merkle_tree::merkle_root::<
			<T as pallet_mmr::Config>::Hashing,
			_,
		>(leaves)
		.into();
	}

	pub fn initialize_acss_params(params: &[u8]) {
		if !params.is_empty() {
			assert!(<ACSSParameters<T>>::get().is_empty(), "ACSS params are already initialized!");
			<ACSSParameters<T>>::put(params);
		}
	}

	/// Return current authorities length.
	pub fn authorities_len() -> usize {
		Authorities::<T>::decode_len().unwrap_or(0)
	}

	/// Get the current slot from the pre-runtime digests.
	pub fn current_predigest_from_digests() -> Option<PreDigest> {
		frame_system::Pallet::<T>::digest()
			.logs
			.iter()
			.filter_map(|d| d.as_pre_runtime())
			.filter_map(|(id, mut data)| {
				if id == AURA_ENGINE_ID {
					PreDigest::decode(&mut data).ok()
				} else {
					None
				}
		}).next()
	}
	
	/// Determine the Aura slot-duration based on the Timestamp module configuration.
	pub fn slot_duration() -> T::Moment {
		<T as pallet_timestamp::Config>::MinimumPeriod::get().saturating_mul(2u32.into())
	}

	/// Ensure the correctness of the state of this pallet.
	///
	/// This should be valid before or after each state transition of this pallet.
	///
	/// # Invariants
	///
	/// ## `CurrentSlot`
	///
	/// If we don't allow for multiple blocks per slot, then the current slot must be less than the
	/// maximal slot number. Otherwise, it can be arbitrary.
	///
	/// ## `Authorities`
	///
	/// * The authorities must be non-empty.
	/// * The current authority cannot be disabled.
	/// * The number of authorities must be less than or equal to `T::MaxAuthorities`. This however,
	///   is guarded by the type system.
	#[cfg(any(test, feature = "try-runtime"))]
	pub fn do_try_state() -> Result<(), sp_runtime::TryRuntimeError> {
		// We don't have any guarantee that we are already after `on_initialize` and thus we have to
		// check the current slot from the digest or take the last known slot.
		let current_slot = 
			Self::current_predigest_from_digests()
				.map_or_else(|| CurrentSlot::<T>::get(), |predigest| predigest.slot);
		// Check that the current slot is less than the maximal slot number, unless we allow for
		// multiple blocks per slot.
		if !T::AllowMultipleBlocksPerSlot::get() {
			frame_support::ensure!(
				current_slot < u64::MAX,
				"Current slot has reached maximum value and cannot be incremented further.",
			);
		}

		let authorities_len =
			<Authorities<T>>::decode_len().ok_or("Failed to decode authorities length")?;

		// Check that the authorities are non-empty.
		frame_support::ensure!(!authorities_len.is_zero(), "Authorities must be non-empty.");

		// Check that the current authority is not disabled.
		let authority_index = *current_slot % authorities_len as u64;
		frame_support::ensure!(
			!T::DisabledValidators::is_disabled(authority_index as u32),
			"Current validator is disabled and should not be attempting to author blocks.",
		);

		Ok(())
	}

	/// add a new slot secret to runtime storage
	/// we only store the secret here, not the entire proof (in block header)
	pub fn add_slot_secret(slot: Slot, secret: OpaqueSecret) {
		SlotSecrets::<T>::insert(slot, secret);
	}

	/// a helper method to set the current slot
	pub fn set_current_slot(new_slot: Slot) {
		CurrentSlot::<T>::put(new_slot);
	}

}

impl<T: Config> sp_runtime::BoundToRuntimeAppPublic for Pallet<T> {
	type Public = T::AuthorityId;
}

impl<T: Config> OneSessionHandler<T::AccountId> for Pallet<T> {
	type Key = T::AuthorityId;

	#[cfg(feature = "std")]
	fn on_genesis_session<'a, I: 'a>(validators: I)
	where
		I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
	{
		let authorities = validators.map(|(_, k)| k).collect::<Vec<_>>();
		Self::initialize_authorities(&authorities);
	}

	fn on_new_session<'a, I: 'a>(changed: bool, validators: I, _queued_validators: I)
	where
		I: Iterator<Item = (&'a T::AccountId, T::AuthorityId)>,
	{
		// instant changes
		if changed {
			// TODO: at this point, the round of ACSS must be completed in order to procede
			// so we just need to verify that it completed and we're good to go
			let next_authorities = validators.map(|(_, k)| k).collect::<Vec<_>>();
			let last_authorities = Self::authorities();
			if last_authorities != next_authorities {
				if next_authorities.len() as u32 > T::MaxAuthorities::get() {
					log::warn!(
						target: LOG_TARGET,
						"next authorities list larger than {}, truncating",
						T::MaxAuthorities::get(),
					);
				}
				let bounded = <BoundedVec<_, T::MaxAuthorities>>::truncate_from(next_authorities);
				Self::change_authorities(bounded);
			}
		}
	}

	fn on_disabled(i: u32) {
		let log = DigestItem::Consensus(
			AURA_ENGINE_ID,
			ConsensusLog::<T::AuthorityId>::OnDisabled(i as AuthorityIndex).encode(),
		);

		<frame_system::Pallet<T>>::deposit_log(log);
	}
}


// impl<T: Config> pallet_session::ShouldEndSession<BlockNumberFor<T>> for Pallet<T> {
// 	fn should_end_session(now: BlockNumberFor<T>) -> bool {
// 		// it might be (and it is in current implementation) that session module is calling
// 		// `should_end_session` from it's own `on_initialize` handler, in which case it's
// 		// possible that babe's own `on_initialize` has not run yet, so let's ensure that we
// 		// have initialized the pallet and updated the current slot.
// 		// Self::initialize(now);
// 		// Self::should_epoch_change(now)
// 	}
// }

impl<T: Config> FindAuthor<u32> for Pallet<T> {
	fn find_author<'a, I>(digests: I) -> Option<u32>
	where
		I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
	{
		for (id, mut data) in digests.into_iter() {
			if id == AURA_ENGINE_ID {
				let slot = Slot::decode(&mut data).ok()?;
				let author_index = *slot % Self::authorities_len() as u64;
				return Some(author_index as u32)
			}
		}

		None
	}
}

/// We can not implement `FindAuthor` twice, because the compiler does not know if
/// `u32 == T::AuthorityId` and thus, prevents us to implement the trait twice.
#[doc(hidden)]
pub struct FindAccountFromAuthorIndex<T, Inner>(sp_std::marker::PhantomData<(T, Inner)>);

impl<T: Config, Inner: FindAuthor<u32>> FindAuthor<T::AuthorityId>
	for FindAccountFromAuthorIndex<T, Inner>
{
	fn find_author<'a, I>(digests: I) -> Option<T::AuthorityId>
	where
		I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
	{
		let i = Inner::find_author(digests)?;

		let validators = <Pallet<T>>::authorities();
		validators.get(i as usize).cloned()
	}
}

/// Find the authority ID of the Aura authority who authored the current block.
pub type AuraAuthorId<T> = FindAccountFromAuthorIndex<T, Pallet<T>>;

impl<T: Config> IsMember<T::AuthorityId> for Pallet<T> {
	fn is_member(authority_id: &T::AuthorityId) -> bool {
		Self::authorities().iter().any(|id| id == authority_id)
	}
}

impl<T: Config> OnTimestampSet<T::Moment> for Pallet<T> {
	fn on_timestamp_set(moment: T::Moment) {
		let slot_duration = Self::slot_duration();
		assert!(!slot_duration.is_zero(), "Aura slot duration cannot be zero.");

		let timestamp_slot = moment / slot_duration;
		let timestamp_slot = Slot::from(timestamp_slot.saturated_into::<u64>());

		assert_eq!(
			CurrentSlot::<T>::get(),
			timestamp_slot,
			"Timestamp slot must match `CurrentSlot`"
		);
	}
}

// pub trait SlotSecretProvider {
// 	/// get the latest (current) slot secret
// 	fn get() -> Option<OpaqueSecret>;
// }

// impl<T: Config> SlotSecretProvider for Pallet<T> {
// 	fn get() -> Option<OpaqueSecret> {
// 		SlotSecrets::<T>::get(Self::current_slot())
// 	}
// }

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
		if let Some(secret) = SlotSecrets::<T>::get(Self::current_slot()) {
			let (_, p, _) = T::IBEParamProvider::get();
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
