use crate as pallet_etf;
use frame_support::traits::ConstU64;
use sp_core::{ConstU32, H256};
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
};
use frame_support_test::TestRandomness;
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balances,
		Etf: pallet_etf,
	}
);

type AccountId = u64;

impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Nonce = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u64>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<3>;
}

impl pallet_balances::Config for Test {
	type Balance = u64;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU64<1>;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type FreezeIdentifier = ();
	type MaxHolds = ConstU32<10>;
	type MaxFreezes = ();
}

impl pallet_etf::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = pallet_etf::weights::SubstrateWeightInfo<Test>;
    type Randomness = TestRandomness<Self>;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext(
    g1_hex: &str,
	g2_hex: &str,
) -> sp_io::TestExternalities {

	let mut storage = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

    let ibe_g1: Vec<u8> = array_bytes::hex2bytes_unchecked(g1_hex);
	let ibe_g2: Vec<u8> = array_bytes::hex2bytes_unchecked(g2_hex);

    let config: pallet_etf::GenesisConfig::<Test> = pallet_etf::GenesisConfig {
		initial_ibe_params: ibe_g1.clone(),
		initial_ibe_pp: ibe_g2.clone(),
		initial_ibe_commitment: ibe_g2.clone(),
		_config:Default::default(),
	};

	config.assimilate_storage(&mut storage).unwrap();
    let mut ext: sp_io::TestExternalities = storage.into();
	// Clear thread local vars for https://github.com/paritytech/substrate/issues/10479.
	// ext.execute_with(|| take_hooks());
	ext.execute_with(|| System::set_block_number(1));
	ext
}
