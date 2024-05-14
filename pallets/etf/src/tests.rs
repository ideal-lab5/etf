// // This file is part of Substrate.

// // Copyright (C) Parity Technologies (UK) Ltd.
// // SPDX-License-Identifier: Apache-2.0

// // Licensed under the Apache License, Version 2.0 (the "License");
// // you may not use this file except in compliance with the License.
// // You may obtain a copy of the License at
// //
// // 	http://www.apache.org/licenses/LICENSE-2.0
// //
// // Unless required by applicable law or agreed to in writing, software
// // distributed under the License is distributed on an "AS IS" BASIS,
// // WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// // See the License for the specific language governing permissions and
// // limitations under the License.
// use codec::Encode;
// use std::vec;

// use frame_support::{
// 	assert_err, assert_ok,
// 	dispatch::{GetDispatchInfo, Pays},
// 	traits::{Currency, KeyOwnerProofSystem, OnInitialize},
// };
// use sp_consensus_beefy_etf::{
// 	check_equivocation_proof,
// 	known_payloads::MMR_ROOT_ID,
// 	test_utils::{generate_equivocation_proof, Keyring as BeefyKeyring},
// 	Payload, ValidatorSet, KEY_TYPE as BEEFY_KEY_TYPE,
// };
// use sp_runtime::DigestItem;

// use crate::{self as etf, mock::*, Call, Config, Error, Weight, WeightInfo};

// // fn init_block(block: u64) {
// // 	System::set_block_number(block);
// // 	Session::on_initialize(block);
// // }

// // pub fn beefy_log(log: ConsensusLog<BeefyId>) -> DigestItem {
// // 	DigestItem::Consensus(BEEFY_ENGINE_ID, log.encode())
// // }

// #[test]
// fn genesis_session_initializes_resharing_and_commitments_with_valid_values() {
// 	let genesis_resharing = mock_resharing(
// 		vec![
// 			(1, 2, vec![1, 2]), 
// 			(3, 4, vec![3, 4])
// 		]);

// 	let want_resharing = genesis_resharing.clone();
// 	let genesis_roundkey = [1;96].to_vec();

// 	ExtBuilder::default()
// 		.add_authorities(mock_authorities(vec![1, 3]))
// 		.add_resharing(genesis_resharing)
// 		.add_round_key(genesis_roundkey)
// 		.build_and_execute(|| 
// 	{
// 		// resharings are populated
// 		let resharings = etf::Shares::<Test>::get();
// 		assert_eq!(resharings.len(), 2);
// 		assert_eq!(resharings[0], want_resharing[0].2);
// 		assert_eq!(resharings[1], want_resharing[1].2);

// 		let commitments = etf::Commitments::<Test>::get();
// 		assert_eq!(commitments.len(), 2);
// 		assert_eq!(commitments[0], want_resharing[0].1);
// 		assert_eq!(commitments[1], want_resharing[1].1);
// 	});
// }