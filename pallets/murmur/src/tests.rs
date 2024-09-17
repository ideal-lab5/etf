use crate::{self as murmur, mock::*, Error, Murmur, Proxy, RuntimeOrigin};
use frame_system::Call as SystemCall;
use ark_std::{test_rng, UniformRand};
use ark_serialize::CanonicalSerialize;
use frame_support::{assert_noop, assert_ok};

use sha3::Digest;

use murmur_core::types::MergeLeaves;

use ckb_merkle_mountain_range::{
    MerkleProof,
    util::{
        MemMMR,
        MemStore
    },
};

#[test]
fn it_can_create_new_proxy_with_unique_name() {
	let unique_name = "name".to_vec();
	let root = vec![1,2,3];
	let size = 0;

	new_test_ext().execute_with(|| {
		assert_ok!(Murmur::create(
			RuntimeOrigin::signed(1),
			root,
			size,
			unique_name
		));

		// check storage
		let registered_proxy = murmur::Registry::<Test>::get(unique_name.clone());
		assert!(registered_proxy.is_some());
		// verify data
		assert_eq!(registered_proxy.root, root);
		assert_eq!(registered_proxy.size, size);
		assert_eq!(registered_proxy.name, unique_name);
	});
}

fn call_remark(value: Vec<u8>) -> RuntimeCall {
	RuntimeCall::System(SystemCall::remark { value })
}

// Q: This would be much easier if we could import the 'client' features from murmur-core for tests. How can I do that?
#[test]
fn it_can_proxy_valid_calls() {
	let unique_name = "name".to_vec();
	// let root = vec![1,2,3];
	let size = 1;

	// generate otp code for block 2
	let fake_otp_at_block_2 = vec![2,6,5,7,7,0];
	// build a real mmr
	let leaves = vec![fake_otp_at_block_2];

    // let mut mmr_store_file = File::create("mmr_store").unwrap();
    let store = MemStore::default();
    let mut mmr = MemMMR::<_, MergeLeaves>::new(0, store);

    // TODO: HKDF? just hash the seed?
    let ephem_msk = [1;32];

	// populate MMR
	leaves.iter().for_each(|leaf| {
		// TODO: error handling
		mmr.push(leaf.1.clone()).unwrap();
	});

	let call = Box::new(call_remark(vec![1,2,3]));

	let mut hasher = sha3::Sha3_256::default();
    hasher.update(fake_otp_at_block_2);
    hasher.update(&call.encode());
    let hash = hasher.finalize().to_vec();

	let root = mmr.get_root().expect("The MMR root should be calculable");
	// then prepare a merkle proof
	let proof = mmr.gen_proof(vec![0]).expect("should be ok");
	let proof_items: Vec<Vec<u8>> = proof.proof_items().iter()
        .map(|leaf| leaf.0.to_vec().clone())
        .collect::<Vec<_>>();

	new_test_ext().execute_with(|| {
		// create the named murmur wallet
		assert_ok!(Murmur::create(
			RuntimeOrigin::signed(1),
			root.0,
			size,
			unique_name
		));
		// tlock decryption is just passthrough, so target == otp
		// then to verify the mmr, we should construct it using the plaintext
		
		// recall: dummy tlock provider says latest = 2, so the call should work
		assert_ok!(Murmur::execute(
			RuntimeOrigin::signed(2),
			unique_name,
			0,
			fake_otp_at_block_2,
			hash,
			proof_items,
			call,
		));

	});
}
