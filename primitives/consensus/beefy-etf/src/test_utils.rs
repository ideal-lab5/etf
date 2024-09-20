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

use crate::{
	bls_crypto, AuthorityIdBound, BeefySignatureHasher, Commitment, EquivocationProof, Payload,
	ValidatorSetId, VoteMessage, bls_crypto::{AuthorityId as BeefyId},
};
use sp_application_crypto::{AppCrypto, AppPair, RuntimeAppPublic, Wraps, UncheckedFrom};
use sp_core::{bls377, Pair};
use sp_runtime::traits::Hash;

use codec::Encode;
use std::{collections::HashMap, marker::PhantomData};
use strum::IntoEnumIterator;


use ark_serialize::CanonicalSerialize;
use ark_std::UniformRand;
use etf_crypto_primitives::{
	proofs::hashed_el_gamal_sigma::BatchPoK,
	dpss::acss::DoubleSecret
};
use rand::rngs::OsRng;
use w3f_bls::{DoublePublicKey, DoublePublicKeyScheme, EngineBLS, SerializableToBytes, TinyBLS377};

/// Set of test accounts using [`crate::bls_crypto`] types.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, strum::EnumIter)]
pub enum Keyring<AuthorityId> {
	Alice,
	Bob,
	Charlie,
	Dave,
	Eve,
	Ferdie,
	One,
	Two,
	_Marker(PhantomData<AuthorityId>),
}

/// Trait representing BEEFY specific generation and signing behavior of authority id
///
/// Accepts custom hashing fn for the message and custom convertor fn for the signer.
pub trait BeefySignerAuthority<MsgHash: Hash>: AppPair {
	/// Generate and return signature for `message` using custom hashing `MsgHash`
	fn sign_with_hasher(&self, message: &[u8]) -> <Self as AppCrypto>::Signature;
}

impl<MsgHash> BeefySignerAuthority<MsgHash> for <bls_crypto::AuthorityId as AppCrypto>::Pair
where
	MsgHash: Hash,
	<MsgHash as Hash>::Output: Into<[u8; 32]>,
{
	fn sign_with_hasher(&self, message: &[u8]) -> <Self as AppCrypto>::Signature {
		self.as_inner_ref().sign(&message).into()
	}
}

/// Implement Keyring functionalities generically over AuthorityId
impl<AuthorityId> Keyring<AuthorityId>
where
	AuthorityId: AuthorityIdBound + From<<<AuthorityId as AppCrypto>::Pair as AppCrypto>::Public>,
	<AuthorityId as AppCrypto>::Pair: BeefySignerAuthority<BeefySignatureHasher>,
	<AuthorityId as RuntimeAppPublic>::Signature:
		Send + Sync + From<<<AuthorityId as AppCrypto>::Pair as AppCrypto>::Signature>,
{
	/// Sign `msg`.
	pub fn sign(&self, msg: &[u8]) -> <AuthorityId as RuntimeAppPublic>::Signature {
		let key_pair: <AuthorityId as AppCrypto>::Pair = self.pair();
		key_pair.sign_with_hasher(&msg).into()
	}

	/// Return key pair.
	pub fn pair(&self) -> <AuthorityId as AppCrypto>::Pair {
		<AuthorityId as AppCrypto>::Pair::from_string(self.to_seed().as_str(), None)
			.unwrap()
			.into()
	}

	/// Return public key.
	pub fn public(&self) -> AuthorityId {
		self.pair().public().into()
	}

	/// Return seed string.
	pub fn to_seed(&self) -> String {
		format!("//{}", self)
	}

	/// Get Keyring from public key.
	pub fn from_public(who: &AuthorityId) -> Option<Keyring<AuthorityId>> {
		Self::iter().find(|k| k.public() == *who)
	}
}

lazy_static::lazy_static! {
	static ref PRIVATE_KEYS: HashMap<Keyring<bls_crypto::AuthorityId>, bls_crypto::Pair> =
		Keyring::iter().map(|i| (i.clone(), i.pair())).collect();
	static ref PUBLIC_KEYS: HashMap<Keyring<bls_crypto::AuthorityId>, bls_crypto::Public> =
		PRIVATE_KEYS.iter().map(|(name, pair)| (name.clone(), sp_application_crypto::Pair::public(pair))).collect();
}

impl From<Keyring<bls_crypto::AuthorityId>> for bls_crypto::Pair {
	fn from(k: Keyring<bls_crypto::AuthorityId>) -> Self {
		k.pair()
	}
}

impl From<Keyring<bls_crypto::AuthorityId>> for bls377::Pair {
	fn from(k: Keyring<bls_crypto::AuthorityId>) -> Self {
		k.pair().into()
	}
}

impl From<Keyring<bls_crypto::AuthorityId>> for bls_crypto::Public {
	fn from(k: Keyring<bls_crypto::AuthorityId>) -> Self {
		(*PUBLIC_KEYS).get(&k).cloned().unwrap()
	}
}

/// Create a new `EquivocationProof` based on given arguments.
pub fn generate_equivocation_proof(
	vote1: (u64, Payload, ValidatorSetId, &Keyring<bls_crypto::AuthorityId>),
	vote2: (u64, Payload, ValidatorSetId, &Keyring<bls_crypto::AuthorityId>),
) -> EquivocationProof<u64, bls_crypto::Public, bls_crypto::Signature> {
	let signed_vote = |block_number: u64,
	                   payload: Payload,
	                   validator_set_id: ValidatorSetId,
	                   keyring: &Keyring<bls_crypto::AuthorityId>| {
		let commitment = Commitment { validator_set_id, block_number, payload };
		let signature = keyring.sign(&commitment.encode());
		VoteMessage { commitment, id: keyring.public(), signature }
	};
	let first = signed_vote(vote1.0, vote1.1, vote1.2, vote1.3);
	let second = signed_vote(vote2.0, vote2.1, vote2.2, vote2.3);
	EquivocationProof { first, second }
}

/// Helper function to prepare initial secrets and resharing for ETF conensus
/// return a vec of (authority id, resharing, pubkey commitment) along with ibe public key against the master secret
pub fn etf_genesis<E: EngineBLS>(
    initial_authorities: Vec<BeefyId>,
) -> (Vec<u8>, Vec<(BeefyId, Vec<u8>)>) {
    let msk_prime = E::Scalar::rand(&mut OsRng);
    let keypair = w3f_bls::KeypairVT::<E>::generate(&mut OsRng);
    let msk: E::Scalar = keypair.secret.0;
    let double_public: DoublePublicKey<E> = DoublePublicKey(
        keypair.into_public_key_in_signature_group().0,
        keypair.public.0,
    );

    let double_secret = DoubleSecret::<E>(msk, msk_prime);

    let mut double_public_bytes = Vec::new();
    double_public
        .serialize_compressed(&mut double_public_bytes)
        .unwrap();

    let genesis_resharing: Vec<(
		DoublePublicKey<E>,
		BatchPoK<E::PublicKeyGroup>
	)> = double_secret
        .reshare(
            &initial_authorities
                .iter()
                .map(|authority| {
                    w3f_bls::single::PublicKey::<E>(
                        w3f_bls::double::DoublePublicKey::<E>::from_bytes(&authority.to_raw_vec())
                            .unwrap()
                            .1,
                    )
                })
                .collect::<Vec<_>>(),
            initial_authorities.len() as u8, // threshold = full set of authorities for now
            &mut OsRng,
        ).unwrap();

	let serialized_resharings = initial_authorities
        .iter()
        .enumerate()
        .map(|(idx, _)| {
			let pk = &genesis_resharing[idx].0;
			let mut pk_bytes = [0u8;144];
			pk.serialize_compressed(&mut pk_bytes[..]).unwrap();
			let blspk = bls377::Public::unchecked_from(pk_bytes);

            let pok = &genesis_resharing[idx].1;
            let mut bytes = Vec::new();
			pok.serialize_compressed(&mut bytes).unwrap();
			
			let beefy_id = BeefyId::from(blspk);
			(beefy_id, bytes)
        })
        .collect::<Vec<_>>();
    (double_public_bytes, serialized_resharings)
}