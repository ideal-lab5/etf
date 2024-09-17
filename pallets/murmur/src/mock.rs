use crate as pallet_murmur;
use frame_support::traits::{ConstBool, ConstU64};
use sp_core::{ConstU32, H256};
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
};
use sp_runtime::BuildStorage;
use etf_crypto_primitives::encryption::tlock::DecryptionResult;
use pallet_randomness_beacon::TimelockEncryptionProvider;

type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balance,
        Proxy: pallet_proxy,
        Murmur: pallet_murmur,
		// Balances: pallet_balances,
		// Aura: pallet_etf_aura,
		// Etf: pallet_etf,
	}
);

type AccountId = u64;

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Block = Block;
	type AccountData = pallet_balances::AccountData<u64>;
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

impl pallet_proxy::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type ProxyType = ();
	type ProxyDepositBase = ConstU64<1>;
	type ProxyDepositFactor = ConstU64<1>;
	type MaxProxies = ConstU32<32>;
	type WeightInfo = ();
	type MaxPending = ConstU32<32>;
	type CallHasher = BlakeTwo256;
	type AnnouncementDepositBase = ConstU64<1>;
	type AnnouncementDepositFactor = ConstU64<1>;
}

/// a passthrough dummy tlock provider
/// doesn't actually do anything, just passes the ciphertext as the plaintext
pub struct DummyTlock;
impl TimelockEncryptionProvider<u64> for DummyTlock {
    fn decrypt_at(
        passthrough: &[u8], 
        block_number: u64
    ) -> Result<etf_crypto_primitives::encryption::tlock::DecryptionResult, pallet_randomness_beacon::TimelockError> {
        let result = etf_crypto_primitives::encryption::tlock::DecryptionResult {
            message: passthrough.clone().to_vec(),
            secret: [0;32],
        };
        Ok(result)
    }

	fn latest() -> u64 {
		2u64
	}
}

impl pallet_murmur::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type TlockProvider = DummyTlock;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut storage = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

	// config.assimilate_storage(&mut storage).unwrap();
    let mut ext: sp_io::TestExternalities = storage.into();
	// Clear thread local vars for https://github.com/paritytech/substrate/issues/10479.
	// ext.execute_with(|| take_hooks());
	ext.execute_with(|| System::set_block_number(1));
	ext
}
