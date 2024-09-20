/*
 * Copyright 2024 by Ideal Labs, LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use super::*;
use std::vec;

use codec::Encode;
use frame_support::{
	construct_runtime, derive_impl, parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, Contains, InstanceFilter},
};
use frame_system::Call as SystemCall;
use sp_consensus_beefy_etf::{mmr::MmrLeafVersion, test_utils::etf_genesis};
use sp_core::Pair;
use sp_io::TestExternalities;
use sp_runtime::{
	app_crypto::bls377::Public,
	impl_opaque_keys,
	testing::TestXt,
	traits::{BlakeTwo256, ConvertInto, Keccak256, OpaqueKeys},
	BuildStorage,
};
use murmur_core::otp::BOTPGenerator;
use sp_state_machine::BasicExternalities;
use etf_crypto_primitives::encryption::tlock::DecryptionResult;

use crate as pallet_murmur;

pub use sp_consensus_beefy_etf::{bls_crypto::AuthorityId as BeefyId, mmr::BeefyDataProvider};

impl_opaque_keys! {
	pub struct MockSessionKeys {
		pub dummy: pallet_beefy_etf::Pallet<Test>,
	}
}

type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Session: pallet_session,
		Mmr: pallet_mmr,
		RandomnessBeacon: pallet_randomness_beacon,
		Etf: pallet_etf,
		Beefy: pallet_beefy_etf,
		BeefyMmr: pallet_beefy_mmr_etf,
		Balances: pallet_balances,
		Proxy: pallet_proxy,
		Murmur: pallet_murmur,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Block = Block;
	type AccountData = pallet_balances::AccountData<u128>;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Test
where
	RuntimeCall: From<C>,
{
	type OverarchingCall = RuntimeCall;
	type Extrinsic = TestXt<RuntimeCall, ()>;
}

impl pallet_session::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = u64;
	type ValidatorIdOf = ConvertInto;
	type ShouldEndSession = pallet_session::PeriodicSessions<ConstU64<1>, ConstU64<0>>;
	type NextSessionRotation = pallet_session::PeriodicSessions<ConstU64<1>, ConstU64<0>>;
	type SessionManager = MockSessionManager;
	type SessionHandler = <MockSessionKeys as OpaqueKeys>::KeyTypeIdProviders;
	type Keys = MockSessionKeys;
	type WeightInfo = ();
}

// pub type MmrLeaf = sp_consensus_beefy_etf::mmr::MmrLeaf<
// 	frame_system::pallet_prelude::BlockNumberFor<Test>,
// 	<Test as frame_system::Config>::Hash,
// 	pallet_beefy_mmr::MerkleRootOf<Test>,
// 	Vec<u8>,
// >;

impl pallet_mmr::Config for Test {
	const INDEXING_PREFIX: &'static [u8] = b"mmr";
	type Hashing = Keccak256;
	type LeafData = BeefyMmr;
	type OnNewRoot = pallet_beefy_mmr_etf::DepositBeefyDigest<Test>;
	type WeightInfo = ();
	type BlockHashProvider = pallet_mmr::DefaultBlockHashProvider<Test>;
}

impl pallet_etf::Config for Test {
	type BeefyId = BeefyId;
	type MaxAuthorities = ConstU32<100>;
}

impl pallet_beefy_etf::Config for Test {
	type BeefyId = BeefyId;
	type MaxAuthorities = ConstU32<100>;
	type MaxNominators = ConstU32<1000>;
	type MaxSetIdSessionEntries = ConstU64<100>;
	type OnNewValidatorSet = BeefyMmr;
	type WeightInfo = ();
	type KeyOwnerProof = sp_core::Void;
	type EquivocationReportSystem = ();
	type RoundCommitmentProvider = Etf;
}

parameter_types! {
	pub LeafVersion: MmrLeafVersion = MmrLeafVersion::new(1, 5);
}

impl pallet_beefy_mmr_etf::Config for Test {
	type LeafVersion = LeafVersion;

	type BeefyAuthorityToMerkleLeaf = pallet_beefy_mmr_etf::BeefyBlsToEthereum;

	type LeafExtra = Vec<u8>;

	type BeefyDataProvider = ();
}

pub struct DummyDataProvider;
impl BeefyDataProvider<Vec<u8>> for DummyDataProvider {
	fn extra_data() -> Vec<u8> {
		let mut col = vec![(15, vec![1, 2, 3]), (5, vec![4, 5, 6])];
		col.sort();
		binary_merkle_tree::merkle_root::<<Test as pallet_mmr::Config>::Hashing, _>(
			col.into_iter().map(|pair| pair.encode()),
		)
		.as_ref()
		.to_vec()
	}
}

impl pallet_balances::Config for Test {
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type Balance = u128;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = System;
	type WeightInfo = ();
	type RuntimeHoldReason = ();
	type RuntimeFreezeReason = ();
	type FreezeIdentifier = ();
	type MaxFreezes = ();
}

impl pallet_randomness_beacon::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MaxPulses = ConstU32<256000>;
}

pub struct DummyTlockProvider;
impl TimelockEncryptionProvider<u64> for DummyTlockProvider {

	fn decrypt_at(bytes: &[u8], when: u64) -> Result<DecryptionResult, pallet_randomness_beacon::TimelockError> {
		let seed = b"seed".to_vec();
		let mut hasher = sha3::Sha3_256::default();
		hasher.update(seed);
		let hash = hasher.finalize();
		let generator = BOTPGenerator::new(hash.to_vec());
		let otp_code = generator.generate(when as u32);

		Ok(DecryptionResult {
			message: otp_code.as_bytes().to_vec(),
			secret: [0;32],
		})
	}

	fn latest() -> u64 {
		1
	}
}

impl pallet_murmur::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type TlockProvider = DummyTlockProvider;
}

#[derive(
	Copy,
	Clone,
	Eq,
	PartialEq,
	Ord,
	PartialOrd,
	Encode,
	Decode,
	RuntimeDebug,
	MaxEncodedLen,
	scale_info::TypeInfo,
)]
pub enum ProxyType {
	Any,
	JustTransfer,
}
impl Default for ProxyType {
	fn default() -> Self {
		Self::Any
	}
}
impl InstanceFilter<RuntimeCall> for ProxyType {
	fn filter(&self, c: &RuntimeCall) -> bool {
		match self {
			ProxyType::Any => true,
			ProxyType::JustTransfer => {
				matches!(
					c,
					RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death { .. })
				)
			},
		}
	}
	fn is_superset(&self, o: &Self) -> bool {
		self == &ProxyType::Any || self == o
	}
}
pub struct BaseFilter;
impl Contains<RuntimeCall> for BaseFilter {
	fn contains(c: &RuntimeCall) -> bool {
		match *c {
			// Remark is used as a no-op call in the benchmarking
			RuntimeCall::System(SystemCall::remark { .. }) => true,
			RuntimeCall::System(_) => false,
			_ => true,
		}
	}
}

impl pallet_proxy::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type ProxyType = ProxyType;
	type ProxyDepositBase = ConstU128<1>;
	type ProxyDepositFactor = ConstU128<1>;
	type MaxProxies = ConstU32<4>;
	type WeightInfo = ();
	type CallHasher = BlakeTwo256;
	type MaxPending = ConstU32<2>;
	type AnnouncementDepositBase = ConstU128<1>;
	type AnnouncementDepositFactor = ConstU128<1>;
}

pub struct MockSessionManager;
impl pallet_session::SessionManager<u64> for MockSessionManager {
	fn end_session(_: sp_staking::SessionIndex) {}
	fn start_session(_: sp_staking::SessionIndex) {}
	fn new_session(idx: sp_staking::SessionIndex) -> Option<Vec<u64>> {
		if idx == 0 || idx == 1 {
			Some(vec![1, 2])
		} else if idx == 2 {
			Some(vec![3, 4])
		} else {
			None
		}
	}
}

// Note, that we can't use `UintAuthorityId` here. Reason is that the implementation
// of `to_public_key()` assumes, that a public key is 32 bytes long. This is true for
// ed25519 and sr25519 but *not* for ecdsa. A compressed ecdsa public key is 33 bytes,
// with the first one containing information to reconstruct the uncompressed key.
pub fn mock_beefy_id(id: u8) -> BeefyId {
	// generate a new keypair and get the public key
	let kp = sp_core::bls::Pair::from_seed_slice(&[id; 32]).unwrap();
	BeefyId::from(kp.public())
}

pub fn mock_authorities(vec: Vec<u8>) -> Vec<(u64, BeefyId)> {
	vec.into_iter().map(|id| ((id as u64), mock_beefy_id(id))).collect()
}

pub fn new_test_ext(ids: Vec<u8>) -> TestExternalities {
	new_test_ext_raw_authorities(mock_authorities(ids))
}

pub fn new_test_ext_raw_authorities(authorities: Vec<(u64, BeefyId)>) -> TestExternalities {
	let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

	let balances: Vec<_> = (0..authorities.len()).map(|i| (i as u64, 10_000_000)).collect();

	pallet_balances::GenesisConfig::<Test> { balances }
		.assimilate_storage(&mut t)
		.unwrap();

	let session_keys: Vec<_> = authorities
		.iter()
		.enumerate()
		.map(|(_, id)| (id.0 as u64, id.0 as u64, MockSessionKeys { dummy: id.1.clone() }))
		.collect();

	BasicExternalities::execute_with_storage(&mut t, || {
		for (ref id, ..) in &session_keys {
			frame_system::Pallet::<Test>::inc_providers(id);
		}
	});

	let (round_pubkey, genesis_resharing) = etf_genesis::<w3f_bls::TinyBLS377>(
		authorities.iter().map(|(_, id)| id.clone()).collect::<Vec<_>>(),
	);

	pallet_etf::GenesisConfig::<Test> {
		genesis_resharing,
		round_pubkey,
		_phantom: Default::default(),
	}
	.assimilate_storage(&mut t)
	.unwrap();

	pallet_session::GenesisConfig::<Test> { keys: session_keys }
		.assimilate_storage(&mut t)
		.unwrap();

	t.into()
}
