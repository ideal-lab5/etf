use node_runtime::{AccountId, RuntimeGenesisConfig, Signature, WASM_BINARY};
use sc_service::ChainType;
use sp_consensus_etf_aura::sr25519::AuthorityId as AuraId;
use pallet_etf::crypto::Public as ETFId;
use sp_consensus_etf_aura::PEK;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::{sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};
// use etf_crypto_primitives::dpss::acss::Capsule;

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
pub fn authority_keys_from_seed(s: &str, pek: Vec<u8>) -> (AuraId, GrandpaId, ETFId, PEK) {
	(get_from_seed::<AuraId>(s), get_from_seed::<GrandpaId>(s), get_from_seed::<ETFId>(s), pek)
}

// pub fn genesis_shares(hex_bytes: &[u8]) -> Vec<Capsule> {
// 	let bytes = array_bytes::hex2bytes_unchecked(hex_bytes);
// 	let caps: Vec<Capsule> = bincode::deserialize(&bytes).unwrap();
// 	caps
// }

pub fn development_config() -> Result<ChainSpec, String> {
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
			authority_keys_from_seed("Alice", b"7b226e223a223231313932363539343032333830343131383332373530303936303731303434303536323937353135323036323731393237353838303237363237383437363837303834343630343436373235323039393033343934393133393833323437353737333339393139323630353932373439333332343037373337333430353738303334333839393236333731363131393537323637303530353639303430343334363538313239313434373333343331353533333732343635323334303730343235323433393834353331363330383738343633383938343839323837353530363434383434303531363434313436373733333834353735333430383338383730333132313235333239313331363838303339383735373037343532303734333835333034383334313338353434363039303031303634313232323238373937313737323135313934313130303331383436323133373134343532363039343131323033363734373434323430363132353830343337323635303134363230313338353634383735313636313131333831323736383638333135303337313239303136363032373033393734363639353431383333333330383436373837303432303832323331393937323932313639393936333132383938303534313730323335383138363132383632323031373532393331373731363538313233333930313438333334353237323131333332353735383533303536333530363431303537363131383539363130393938333035303739383733373930333030363137393838373737363434393439333436333530393431333737313936373539303431313932393236323937227d".to_vec()), 
		],
		// initial capsules for etf
		Vec::new(),
		Vec::new(),
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
			authority_keys_from_seed("Alice", b"7b226e223a223231313932363539343032333830343131383332373530303936303731303434303536323937353135323036323731393237353838303237363237383437363837303834343630343436373235323039393033343934393133393833323437353737333339393139323630353932373439333332343037373337333430353738303334333839393236333731363131393537323637303530353639303430343334363538313239313434373333343331353533333732343635323334303730343235323433393834353331363330383738343633383938343839323837353530363434383434303531363434313436373733333834353735333430383338383730333132313235333239313331363838303339383735373037343532303734333835333034383334313338353434363039303031303634313232323238373937313737323135313934313130303331383436323133373134343532363039343131323033363734373434323430363132353830343337323635303134363230313338353634383735313636313131333831323736383638333135303337313239303136363032373033393734363639353431383333333330383436373837303432303832323331393937323932313639393936333132383938303534313730323335383138363132383632323031373532393331373731363538313233333930313438333334353237323131333332353735383533303536333530363431303537363131383539363130393938333035303739383733373930333030363137393838373737363434393439333436333530393431333737313936373539303431313932393236323937227d".to_vec()), 
			authority_keys_from_seed("Bob", b"7b226e223a223235353337303431323234353835333731333030313733343439343730393430323934333133333833363636303734343134323238323231363238393931303835313637373636323731373735323536393138383130393734373938353734303732383030353135303937393632313332343238343132323930333431343938343233323437393636313036373834373134363131383535393839393138333133373637343631393337373830323535313431333534353937303737383530323630363538323630303635383232383936303635333539383532333132353635393536383036363231363135333731373938383139313135383933303332363939323438393332363735383039343934313734353136323836373331383837373233333136323134333937313231333134313931303333343037303432323531323330393739383535393632323032303031333635373031363937343636363238323834363831303235303839353536363337313133353836313638303436323236373930363037333237383933373731383237323831323239303534323737363936343731333331303330363638353135353630353436343534383137373237393139313031353133313738343334313338303930313939353538353034343832323834303538373034323337353337363732333432333933393231313130383535323139303234333135313139323339383130323330333637393932323733333432343137343638363138373532373536323237363937313239343935383037353638363339393536363532323634313735303838373231333937333630363237373335393933227d".to_vec()),
		],
		// Initial shares
		Vec::new(),
		Vec::new(),
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

/// Configure initial storage state for FRAME modules.
fn testnet_genesis(
	initial_authorities: Vec<(AuraId, GrandpaId, ETFId, PEK)>,
	shares: Vec<(Vec<u8>, Vec<u8>)>,
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
			"authorities": initial_authorities.iter().map(|x| (x.0.clone())).collect::<Vec<_>>(),
			// "shares": shares,
			// "acssParams": acss_params,
		},
		"grandpa": {
			"authorities": initial_authorities.iter().map(|x| (x.1.clone(), 1)).collect::<Vec<_>>(),
		},
		"sudo": {
			// Assign network admin rights.
			"key": Some(root_key),
		},
		"etf": {
			"authorities": initial_authorities.iter().map(|x| (x.2.clone(), x.3.clone())).collect::<Vec<_>>(),
			"initialIbeParams": array_bytes::hex2bytes_unchecked(
				"a191b705ef18a6e4e5bd4cc56de0b8f94b1f3c908f3e3fcbd4d1dc12eb85059be7e7d801edc1856c8cfbe6d63a681c1f"),
			"initialIbePp": array_bytes::hex2bytes_unchecked(
				"93e02b6052719f607dacd3a088274f65596bd0d09920b61ab5da61bbdc7f5049334cf11213945d57e5ac7d055d042b7e024aa2b2f08f0a91260805272dc51051c6e47ad4fa403b02b4510b647ae3d1770bac0326a805bbefd48056c8c121bdb8"),
			"initialIbeCommitment": array_bytes::hex2bytes_unchecked(
				"b59c88bafc86ecc5043b1ab1e6d2ba81f29318a52a4bcd31f47248c88e27373f8be07894c8ba58353df8b3febf8e28011317199faae08cea851aa16ba00761a0960b97cb26ca9b36d46d26acace64214107f5eec7d91789eb77a0f130a40db49")
		},
	})
}
