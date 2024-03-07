use node_runtime::{AccountId, RuntimeGenesisConfig, Signature, opaque::SessionKeys, WASM_BINARY};
use sc_service::ChainType;
use sp_consensus_etf_aura::sr25519::AuthorityId as AuraId;
use sp_consensus_etf_aura::PEK;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::{sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};

use ark_std::UniformRand;
use ark_serialize::CanonicalSerialize;
use ark_bls12_381::Fr;
use etf_crypto_primitives::{
	BigInt,
	PaillierEncryptionKey,
	dpss::acss::{
		ACSSParams,
		Capsule,
		HighThresholdACSS,
		WrappedEncryptionKey,
	}
};

use curv::arithmetic::traits::Converter;

// The URL for the telemetry server.
// const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<RuntimeGenesisConfig>;

/// Generate a crypto pair from seed.
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

type AccountPublic = <Signature as Verify>::Signer;

/// Generate an account ID from seed.
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Generate an ETF/Aura authority key.
/// pek_bytes: the paillier encryption key bytes (serialized => hex)
pub fn authority_keys_from_seed(s: &str, pek: Vec<u8>) -> (AuraId, GrandpaId, PEK) {
	(get_from_seed::<AuraId>(s), get_from_seed::<GrandpaId>(s), pek)
}

// pub fn genesis_shares(hex_bytes: &[u8]) -> Vec<Capsule> {
// 	let bytes = array_bytes::hex2bytes_unchecked(hex_bytes);
// 	let caps: Vec<Capsule> = bincode::deserialize(&bytes).unwrap();
// 	caps
// }

pub fn development_config() -> Result<ChainSpec, String> {

	let initial_encoded_commitee_keys = vec![
		b"7b226e223a223134323835313532373932363134333137333434333936333830373632363238373938343437323139383633303534373534363535353533333935353132373135353738303330313738323136353638323032363434373239303139323138353833333136323633363030313931333536363134313936373036353632373733393935333433363132313737353338373833373535303532383731363536383835323136303639363232323034383839383736373536353834323131313535373131303931313333363035303131333033333733383937323039313136323034383334313339313138363236333736333231343133383038363933373835323338373938383438323836333333303136323339343536343033303734303634383331343638353430313131393238383134383636363639323738383339363636343131323834333630303031313633323834393135333330343832323638303336363437313833343736383039373330303830343539323630313635303231373036323732373738343534373632383139343135313134313838393338323038313131383139313835383233383134373237303334373937303038373834393434393431343133383539333133343339323735323939363631353633303233393237383431363738383731323231373534303333343839303737323938313336323333383731383138323631383538383331333230393739343131303737333537353039313837393739303839333739333133393530373835313039333233313836313738303630323032343539303736393930303734313636323338333830383931383636323137227d".to_vec()
	];
	// need to decode keys and convert to PaillierEncryptionKey
	let paillier_encryption_keys: Vec<PaillierEncryptionKey> 
		= initial_encoded_commitee_keys.iter().map(|encoded| {
			let bytes = array_bytes::hex2bytes_unchecked(encoded);
			let ek: PaillierEncryptionKey = serde_json::from_slice(&bytes).unwrap();
			ek
	}).collect::<Vec<_>>();
	let etf_params: (Vec<u8>, Vec<(BigInt, Capsule)>) 
		= build_dev_shares(paillier_encryption_keys);

	Ok(ChainSpec::builder(
		WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?,
		None,
	)
	.with_name("Development")
	.with_id("dev")
	.with_chain_type(ChainType::Development)
	.with_genesis_config_patch(testnet_genesis(
		// Initial PoA authorities
		vec![
			authority_keys_from_seed("Alice", b"7b226e223a223134323835313532373932363134333137333434333936333830373632363238373938343437323139383633303534373534363535353533333935353132373135353738303330313738323136353638323032363434373239303139323138353833333136323633363030313931333536363134313936373036353632373733393935333433363132313737353338373833373535303532383731363536383835323136303639363232323034383839383736373536353834323131313535373131303931313333363035303131333033333733383937323039313136323034383334313339313138363236333736333231343133383038363933373835323338373938383438323836333333303136323339343536343033303734303634383331343638353430313131393238383134383636363639323738383339363636343131323834333630303031313633323834393135333330343832323638303336363437313833343736383039373330303830343539323630313635303231373036323732373738343534373632383139343135313134313838393338323038313131383139313835383233383134373237303334373937303038373834393434393431343133383539333133343339323735323939363631353633303233393237383431363738383731323231373534303333343839303737323938313336323333383731383138323631383538383331333230393739343131303737333537353039313837393739303839333739333133393530373835313039333233313836313738303630323032343539303736393930303734313636323338333830383931383636323137227d".to_vec()), 
		],
		// initial shares
		etf_params.1,
		// acss parameters
		etf_params.0,
		// Sudo account
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		// Pre-funded accounts
		vec![
			get_account_id_from_seed::<sr25519::Public>("Alice"),
			get_account_id_from_seed::<sr25519::Public>("Bob"),
			get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
			get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
		],
		true,
	))
	.build())
}

pub fn local_testnet_config() -> Result<ChainSpec, String> {
	Ok(ChainSpec::builder(
		WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?,
		None,
	)
	.with_name("Local Testnet")
	.with_id("local_testnet")
	.with_chain_type(ChainType::Local)
	.with_genesis_config_patch(testnet_genesis(
		// Initial PoA authorities
		vec![
			authority_keys_from_seed("Alice", b"7b226e223a223134323835313532373932363134333137333434333936333830373632363238373938343437323139383633303534373534363535353533333935353132373135353738303330313738323136353638323032363434373239303139323138353833333136323633363030313931333536363134313936373036353632373733393935333433363132313737353338373833373535303532383731363536383835323136303639363232323034383839383736373536353834323131313535373131303931313333363035303131333033333733383937323039313136323034383334313339313138363236333736333231343133383038363933373835323338373938383438323836333333303136323339343536343033303734303634383331343638353430313131393238383134383636363639323738383339363636343131323834333630303031313633323834393135333330343832323638303336363437313833343736383039373330303830343539323630313635303231373036323732373738343534373632383139343135313134313838393338323038313131383139313835383233383134373237303334373937303038373834393434393431343133383539333133343339323735323939363631353633303233393237383431363738383731323231373534303333343839303737323938313336323333383731383138323631383538383331333230393739343131303737333537353039313837393739303839333739333133393530373835313039333233313836313738303630323032343539303736393930303734313636323338333830383931383636323137227d".to_vec()), 
			authority_keys_from_seed("Bob", b"7b226e223a223134323835313532373932363134333137333434333936333830373632363238373938343437323139383633303534373534363535353533333935353132373135353738303330313738323136353638323032363434373239303139323138353833333136323633363030313931333536363134313936373036353632373733393935333433363132313737353338373833373535303532383731363536383835323136303639363232323034383839383736373536353834323131313535373131303931313333363035303131333033333733383937323039313136323034383334313339313138363236333736333231343133383038363933373835323338373938383438323836333333303136323339343536343033303734303634383331343638353430313131393238383134383636363639323738383339363636343131323834333630303031313633323834393135333330343832323638303336363437313833343736383039373330303830343539323630313635303231373036323732373738343534373632383139343135313134313838393338323038313131383139313835383233383134373237303334373937303038373834393434393431343133383539333133343339323735323939363631353633303233393237383431363738383731323231373534303333343839303737323938313336323333383731383138323631383538383331333230393739343131303737333537353039313837393739303839333739333133393530373835313039333233313836313738303630323032343539303736393930303734313636323338333830383931383636323137227d".to_vec()),
		],
		// Initial shares
		Vec::new(),
		b"97f1d3a73197d7942695638c4fa9ac0fc3688c4f9774b905a14e3a3f171bac586c55e83ff97a1aeffb3af00adb22c6bb80eff466967c0d918e6367ef5f293878f0a350730de2771a4720a7b1878532e32fdcf570a7178ab04340431374f52576".to_vec(),
		// Sudo account
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		// Pre-funded accounts
		vec![
			get_account_id_from_seed::<sr25519::Public>("Alice"),
			get_account_id_from_seed::<sr25519::Public>("Bob"),
			get_account_id_from_seed::<sr25519::Public>("Charlie"),
			get_account_id_from_seed::<sr25519::Public>("Dave"),
			get_account_id_from_seed::<sr25519::Public>("Eve"),
			get_account_id_from_seed::<sr25519::Public>("Ferdie"),
			get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
			get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
			get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
			get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
			get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
			get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
		],
		true,
	))
	.build())
}

fn build_dev_shares(initial_committee: Vec<PaillierEncryptionKey>)
	-> (Vec<u8>, Vec<Capsule>)  {
	// generate msks
	let mut rng = ark_std::test_rng();
	let msk = Fr::rand(&mut rng);
	let msk_prime = Fr::rand(&mut rng);

	let params = ACSSParams::rand(&mut rng);
	// return this 
	let mut params_bytes = Vec::new();
	params.serialize_compressed(&mut params_bytes).unwrap();

	// all authorities required on genesis
	let initial_committee_threshold = initial_committee.len();
	// Vec<(EncryptionKey, Capsule)>
	let shares = HighThresholdACSS::produce_shares(
		params.clone(), 
		msk, 
		msk_prime, 
		&initial_committee.iter().map(|c| WrappedEncryptionKey(c.n.to_bytes().clone())).collect::<Vec<_>>(), 
		initial_committee_threshold as u8, 
		&mut rng,
	);
	(params_bytes, shares)
}

fn session_keys(aura: AuraId, grandpa: GrandpaId) -> SessionKeys {
	SessionKeys { aura, grandpa }
}

/// Configure initial storage state for FRAME modules.
fn testnet_genesis(
	initial_authorities: Vec<(AuraId, GrandpaId, PEK)>,
	shares: Vec<(BigInt, Capsule)>,
	acss_params: Vec<u8>,
	root_key: AccountId,
	endowed_accounts: Vec<AccountId>,
	_enable_println: bool,
) -> serde_json::Value {
	serde_json::json!({
		"balances": {
			// Configure endowed accounts with initial balance of 1 << 60.
			"balances": endowed_accounts.iter().cloned().map(|k| (k, 1u64 << 60)).collect::<Vec<_>>(),
		},
		"aura": {
			"authorities": initial_authorities.iter().map(|x| (x.0.clone(), x.2.clone())).collect::<Vec<_>>(),
			"initialShares": shares,
			"serializedAcssParams": acss_params,
		},
		"grandpa": {
			"authorities": Vec::<(GrandpaId, u64)>::new(),
			// initial_authorities.iter().map(|x| (x.1.clone(), 1)).collect::<Vec<_>>(),
		},
		"sudo": {
			// Assign network admin rights.
			"key": Some(root_key),
		},
		"etf": {
			// "authorities": initial_authorities.iter().map(|x| (x.2.clone(), x.3.clone())).collect::<Vec<_>>(),
			// "shares": shares,
			// "acssParams": acss_params,
			"initialIbeParams": array_bytes::hex2bytes_unchecked(
				"a191b705ef18a6e4e5bd4cc56de0b8f94b1f3c908f3e3fcbd4d1dc12eb85059be7e7d801edc1856c8cfbe6d63a681c1f"),
			"initialIbePp": array_bytes::hex2bytes_unchecked(
				"93e02b6052719f607dacd3a088274f65596bd0d09920b61ab5da61bbdc7f5049334cf11213945d57e5ac7d055d042b7e024aa2b2f08f0a91260805272dc51051c6e47ad4fa403b02b4510b647ae3d1770bac0326a805bbefd48056c8c121bdb8"),
			"initialIbeCommitment": array_bytes::hex2bytes_unchecked(
				"b59c88bafc86ecc5043b1ab1e6d2ba81f29318a52a4bcd31f47248c88e27373f8be07894c8ba58353df8b3febf8e28011317199faae08cea851aa16ba00761a0960b97cb26ca9b36d46d26acace64214107f5eec7d91789eb77a0f130a40db49")
		},
		"session": {
			"keys": initial_authorities.iter().map(|x| {
				(x.0.clone(), x.0.clone(), session_keys(x.0.clone(), x.1.clone()))
			}).collect::<Vec<_>>(),
		}
	})
}
