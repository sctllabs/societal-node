use std::{collections::BTreeMap, str::FromStr};

use polkadot_service::ParaId;
use sc_chain_spec::{ChainSpecExtension, Properties};
use sc_service::ChainType;
use serde::{Deserialize, Serialize};
use societal_node_runtime::{
	wasm_binary_unwrap, AccountId, AuthorityDiscoveryConfig, BalancesConfig,
	CollatorSelectionConfig, CouncilConfig, DaoConfig, DaoEthGovernanceConfig, GenesisConfig,
	GrandpaConfig, ParachainInfoConfig, PolkadotXcmConfig, SessionConfig, SessionKeys, Signature,
	SudoConfig, SystemConfig, EXISTENTIAL_DEPOSIT,
};

#[cfg(feature = "full")]
use {
	societal_node_runtime::{
		chain_config::MaxNominations, Balance, DemocracyConfig, ElectionsConfig, IndicesConfig,
		NominationPoolsConfig, SocietyConfig, StakerStatus, StakingConfig,
		TechnicalCommitteeConfig, DOLLARS,
	},
	sp_runtime::Perbill,
};

use societal_node_runtime::{EVMChainIdConfig, EVMConfig};

use sc_consensus_grandpa::AuthorityId as GrandpaId;
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{crypto::UncheckedInto, sr25519, Pair, Public, H160, U256};
use sp_runtime::traits::{IdentifyAccount, Verify};

const ETH_RPC_URL_TESTNET: &str = "https://goerli.infura.io/v3/9aa3d95b3bc440fa88ea12eaa4456161";

/// The default XCM version to set in genesis config.
const SAFE_XCM_VERSION: u32 = xcm::prelude::XCM_VERSION;

/// Node `ChainSpec` extensions.
///
/// Additional parameters for some Substrate core modules,
/// customizable from the chain spec.
#[derive(Default, Clone, Serialize, Deserialize, ChainSpecExtension)]
pub struct Extensions {
	/// The relay chain of the Parachain.
	pub relay_chain: String,
	/// The id of the Parachain.
	pub para_id: u32,
}

impl Extensions {
	/// Try to get the extension from the given `ChainSpec`.
	pub fn try_get(chain_spec: &dyn sc_service::ChainSpec) -> Option<&Self> {
		sc_chain_spec::get_extension(chain_spec.extensions())
	}
}

/// Specialized `ChainSpec`.
pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig, Extensions>;

// The URL for the telemetry server.
// const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Generate a crypto pair from seed.
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{seed}"), None)
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

/// Helper function to generate stash, controller and session key from seed
pub fn authority_keys_from_seed(
	seed: &str,
) -> (AccountId, AccountId, GrandpaId, AuraId, AuthorityDiscoveryId) {
	(
		get_account_id_from_seed::<sr25519::Public>(&format!("{seed}//stash")),
		get_account_id_from_seed::<sr25519::Public>(seed),
		get_from_seed::<GrandpaId>(seed),
		get_from_seed::<AuraId>(seed),
		get_from_seed::<AuthorityDiscoveryId>(seed),
	)
}

fn properties() -> Properties {
	serde_json::from_str("{\"tokenDecimals\": 12, \"tokenSymbol\": \"SCTL\", \"SS58Prefix\": 1516}")
		.expect("Provided valid json map")
}

fn development_config_genesis() -> GenesisConfig {
	testnet_genesis(
		vec![authority_keys_from_seed("Alice")],
		vec![],
		get_account_id_from_seed::<sr25519::Public>("Alice"),
		None,
		1516,
		2000.into(),
		ETH_RPC_URL_TESTNET,
	)
}

/// Development config (single validator Alice)
pub fn development_config() -> ChainSpec {
	ChainSpec::from_genesis(
		"Societal Development",
		"societal_dev",
		ChainType::Development,
		development_config_genesis,
		vec![],
		None,
		None,
		None,
		Some(properties()),
		// Extensions
		Extensions { relay_chain: "rococo-local".into(), para_id: Default::default() },
	)
}

fn local_testnet_genesis() -> GenesisConfig {
	#[rustfmt::skip]
	// stash, controller, session-key
	// generated with secret:
	// for i in 1 2 3 4 ; do for j in stash controller; do subkey inspect "$secret"//fir//$j//$i; done; done
	//
	// and
	//
	// for i in 1 2 3 4 ; do for j in session; do subkey --ed25519 inspect "$secret"//fir//$j//$i; done; done
	let initial_authorities: Vec<(
		AccountId,
		AccountId,
		GrandpaId,
		AuraId,
		AuthorityDiscoveryId,
	)> = vec![(
		// 5CoLMkimzM8CnWvFcvYsnPNxPf6PjPVxTmvSPTQvrMYit7CY
		array_bytes::hex_n_into_unchecked("0x20832e9244ba5191bd776b47ad8f56e9e3c5213d42bb79042dd907addf45a03f"),
		// 5Ev1mRCYm6XWsT9TezzFVcMemYrkbtvg7J6AxywVeJwmJ1iP
		array_bytes::hex_n_into_unchecked("0x7e13c7c6702bb8fe6d1d2917ea303975f82cc05acb77a9ceece9b1527c380231"),
		// 5G4DFSomE21btAoHbreqFn2WMivJg3vGgYkxLJ3HK3WiRY9t
		array_bytes::hex2array_unchecked("0xb090a211774bd2860bcba90e579e31eb686fc698aa53aa00594e3944919379e5")
			.unchecked_into(),
		// 5EUFiuMACg57Kd1hgPJQre3FY4n3NMhW6F7FndwPKKBiNXAE
		array_bytes::hex2array_unchecked("0x6a6e61d1d594e96cdc94745511c0c431a2369d9963956999c7e29b40b53a4c44")
			.unchecked_into(),
		// 5EUFiuMACg57Kd1hgPJQre3FY4n3NMhW6F7FndwPKKBiNXAE
		array_bytes::hex2array_unchecked("0x6a6e61d1d594e96cdc94745511c0c431a2369d9963956999c7e29b40b53a4c44")
			.unchecked_into(),
	)];

	// generated with secret: subkey inspect "$secret"//fir
	let root_key: AccountId = array_bytes::hex_n_into_unchecked(
		// 5FNv39HixKnNhDab5vAu1Wm7fxP1UgcA5t1V36c3AHFPYwxz
		"0x92981f0715ab7755e8168b4e8b5bdbd08a5a643adb1438eff3873f3197457b4f",
	);

	let endowed_accounts: Vec<AccountId> = vec![root_key.clone()];

	testnet_genesis(
		initial_authorities,
		vec![],
		root_key,
		Some(endowed_accounts),
		1516,
		2000.into(),
		ETH_RPC_URL_TESTNET,
	)
}

fn live_testnet_genesis() -> GenesisConfig {
	#[rustfmt::skip]
	// stash, controller, session-key
	// generated with secret:
	// for i in 1 2 3 4 ; do for j in stash controller; do subkey inspect "$secret"//fir//$j//$i; done; done
	//
	// and
	//
	// for i in 1 2 3 4 ; do for j in session; do subkey --ed25519 inspect "$secret"//fir//$j//$i; done; done
	let initial_authorities: Vec<(
		AccountId,
		AccountId,
		GrandpaId,
		AuraId,
		AuthorityDiscoveryId,
	)> = vec![(
		// 5FvApmAXkALe1YRtfiUr7X4ar5wk8MpBkqSE6uhWPhJM7HEh
		array_bytes::hex_n_into_unchecked("0xaa6e812efbe2fcc309ca614ee85df0f10af789e53a76841a92014fdfdfd2973b"),
		// 5CfqC6hZ1oZRs644d8DpS8Hr8Vwnj5WsADZrh2QtM12nMdgw
		array_bytes::hex_n_into_unchecked("0x1aca47ee7ad89b9019aa2bbe47a56e110eddd390ce4b41e1ac721abf6d63ee6e"),
		// 5H9nC62w1eMkyuBfF1SVBedGJBaHELC1qZRct2znKZKF7dov
		array_bytes::hex2array_unchecked("0xe10bfc8e8d08f82e700520879bd8a3563e78f30f5bc7844d8b3244468c870faa")
			.unchecked_into(),
		// 5DCWpjyj3AcdcMWXSggTDUXYpTnbH4Lzadb21k6QqCw3ez7k
		array_bytes::hex2array_unchecked("0x32310e386bb37fe6ffc3ba8b40ec47c83b35791652f589493ccc8d614b472c7b")
			.unchecked_into(),
		// 5DCWpjyj3AcdcMWXSggTDUXYpTnbH4Lzadb21k6QqCw3ez7k
		array_bytes::hex2array_unchecked("0x32310e386bb37fe6ffc3ba8b40ec47c83b35791652f589493ccc8d614b472c7b")
			.unchecked_into(),
	)];

	// generated with secret: subkey inspect "$secret"//fir
	let root_key: AccountId = array_bytes::hex_n_into_unchecked(
		// 5GmhCNaVnsTtrmNZMg6gb2EJAQQpqgg1GpnLwjCRtdm9ivtQ
		"0xd033c5d047d45a6209f20496dd0aefd5339975c4d54c63d86947c3c07cf8174f",
	);

	let endowed_accounts: Vec<AccountId> = vec![root_key.clone()];

	testnet_genesis(
		initial_authorities,
		vec![],
		root_key,
		Some(endowed_accounts),
		1516,
		2000.into(),
		ETH_RPC_URL_TESTNET,
	)
}

/// Local testnet config (multivalidator Alice + Bob)
pub fn local_testnet_config() -> ChainSpec {
	ChainSpec::from_genesis(
		"Societal Local Testnet",
		"societal_local_testnet",
		ChainType::Local,
		local_testnet_genesis,
		vec![],
		None,
		None,
		None,
		Some(properties()),
		Extensions { relay_chain: "rococo-local".into(), para_id: 2000_u32 },
	)
}

/// Live testnet config
pub fn live_testnet_config() -> ChainSpec {
	ChainSpec::from_genesis(
		"Societal Live Testnet",
		"societal_live_testnet",
		ChainType::Live,
		live_testnet_genesis,
		vec![],
		None,
		None,
		None,
		Some(properties()),
		Extensions { relay_chain: "rococo".into(), para_id: 2000_u32 },
	)
}

fn session_keys(
	grandpa: GrandpaId,
	aura: AuraId,
	authority_discovery: AuthorityDiscoveryId,
) -> SessionKeys {
	SessionKeys { grandpa, aura, authority_discovery }
}

/// Configure initial storage state for FRAME modules.
pub fn testnet_genesis(
	initial_authorities: Vec<(AccountId, AccountId, GrandpaId, AuraId, AuthorityDiscoveryId)>,
	initial_nominators: Vec<AccountId>,
	root_key: AccountId,
	endowed_accounts: Option<Vec<AccountId>>,
	chain_id: u64,
	para_id: ParaId,
	eth_rpc_url: &str,
) -> GenesisConfig {
	#[cfg(feature = "full")]
	const ENDOWMENT: Balance = 10_000_000 * DOLLARS;
	#[cfg(feature = "full")]
	const STASH: Balance = ENDOWMENT / 1000;

	let mut endowed_accounts: Vec<AccountId> = endowed_accounts.unwrap_or_else(|| {
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
		]
	});

	#[cfg(feature = "full")]
	let num_endowed_accounts = endowed_accounts.len();

	// endow all authorities and nominators.
	initial_authorities
		.iter()
		.map(|x| &x.0)
		.chain(initial_nominators.iter())
		.for_each(|x| {
			if !endowed_accounts.contains(x) {
				endowed_accounts.push(x.clone())
			}
		});

	// stakers: all validators and nominators.
	#[cfg(feature = "full")]
	let mut rng = rand::thread_rng();
	#[cfg(feature = "full")]
	let stakers = initial_authorities
		.iter()
		.map(|x| (x.0.clone(), x.1.clone(), STASH, StakerStatus::Validator))
		.chain(initial_nominators.iter().map(|x| {
			use rand::{seq::SliceRandom, Rng};
			let limit = (MaxNominations::get() as usize).min(initial_authorities.len());
			let count = rng.gen::<usize>() % limit;
			let nominations = initial_authorities
				.as_slice()
				.choose_multiple(&mut rng, count)
				.map(|choice| choice.0.clone())
				.collect::<Vec<_>>();
			(x.clone(), x.clone(), STASH, StakerStatus::Nominator(nominations))
		}))
		.collect::<Vec<_>>();

	GenesisConfig {
		system: SystemConfig { code: wasm_binary_unwrap().to_vec() },
		balances: BalancesConfig {
			// Configure endowed accounts with initial balance of 1 << 60.
			balances: endowed_accounts.iter().cloned().map(|k| (k, 1 << 60)).collect(),
		},
		grandpa: GrandpaConfig { authorities: vec![] },
		sudo: SudoConfig {
			// Assign network admin rights.
			key: Some(root_key),
		},
		parachain_info: ParachainInfoConfig { parachain_id: para_id },
		collator_selection: CollatorSelectionConfig {
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			candidacy_bond: EXISTENTIAL_DEPOSIT * 16,
			..Default::default()
		},
		session: SessionConfig {
			keys: initial_authorities
				.iter()
				// .map(|x| (x.0.clone(), x.0.clone(), session_keys(x.2.clone())))
				.map(|x| {
					(x.0.clone(), x.0.clone(), session_keys(x.2.clone(), x.3.clone(), x.4.clone()))
				})
				.collect::<Vec<_>>(),
		},
		#[cfg(feature = "full")]
		staking: StakingConfig {
			validator_count: initial_authorities.len() as u32,
			minimum_validator_count: initial_authorities.len() as u32,
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
			slash_reward_fraction: Perbill::from_percent(10),
			stakers,
			..Default::default()
		},
		council: CouncilConfig::default(),
		aura: Default::default(),
		aura_ext: Default::default(),
		#[cfg(feature = "full")]
		nomination_pools: NominationPoolsConfig {
			min_create_bond: 10 * DOLLARS,
			min_join_bond: DOLLARS,
			..Default::default()
		},
		authority_discovery: AuthorityDiscoveryConfig { keys: vec![] },
		#[cfg(feature = "full")]
		technical_committee: TechnicalCommitteeConfig {
			members: endowed_accounts
				.iter()
				.take((num_endowed_accounts + 1) / 2)
				.cloned()
				.collect(),
			phantom: Default::default(),
		},
		#[cfg(feature = "full")]
		democracy: DemocracyConfig::default(),
		#[cfg(feature = "full")]
		indices: IndicesConfig { indices: vec![] },
		#[cfg(feature = "full")]
		elections: ElectionsConfig {
			members: endowed_accounts
				.iter()
				.take((num_endowed_accounts + 1) / 2)
				.cloned()
				.map(|member| (member, STASH))
				.collect(),
		},
		#[cfg(feature = "full")]
		society: SocietyConfig {
			members: endowed_accounts
				.iter()
				.take((num_endowed_accounts + 1) / 2)
				.cloned()
				.collect(),
			pot: 0,
			max_members: 999,
		},
		// EVM compatibility
		evm_chain_id: EVMChainIdConfig { chain_id },
		evm: EVMConfig {
			accounts: {
				let mut map = BTreeMap::new();
				map.insert(
					// H160 address of CI test runner account
					H160::from_str("6be02d1d3665660d22ff9624b7be0551ee1ac91b")
						.expect("internal H160 is valid; qed"),
					fp_evm::GenesisAccount {
						balance: U256::from_str("0xffffffffffffffffffffffffffffffff")
							.expect("internal U256 is valid; qed"),
						code: Default::default(),
						nonce: Default::default(),
						storage: Default::default(),
					},
				);
				map
			},
		},
		ethereum: Default::default(),
		base_fee: Default::default(),
		dao: {
			DaoConfig { eth_rpc_url: eth_rpc_url.as_bytes().to_vec(), _phantom: Default::default() }
		},
		dao_eth_governance: {
			DaoEthGovernanceConfig {
				eth_rpc_url: eth_rpc_url.as_bytes().to_vec(),
				_phantom: Default::default(),
			}
		},
		parachain_system: Default::default(),
		polkadot_xcm: PolkadotXcmConfig { safe_xcm_version: Some(SAFE_XCM_VERSION) },
	}
}
