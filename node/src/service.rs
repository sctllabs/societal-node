//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

use futures::prelude::*;
use node_primitives::Block;
use sc_cli::SubstrateCli;
use sc_client_api::{BlockBackend, BlockchainEvents};
use sc_consensus_aura::{self, ImportQueueParams, SlotProportion, StartAuraParams};
use sc_executor::NativeElseWasmExecutor;
use sc_network::NetworkService;
use sc_service::{
	config::Configuration, error::Error as ServiceError, BasePath, RpcHandlers, TaskManager,
};
use sc_telemetry::{Telemetry, TelemetryWorker};
use societal_node_runtime::{self, RuntimeApi};
use sp_consensus_aura::sr25519::AuthorityPair as AuraPair;
use sp_runtime::traits::Block as BlockT;
use std::{
	collections::BTreeMap,
	path::PathBuf,
	sync::{Arc, Mutex},
	time::Duration,
};

// Frontier
use crate::cli::Cli;
use fc_consensus::FrontierBlockImport;
use fc_db::Backend as FrontierBackend;
use fc_mapping_sync::{MappingSyncWorker, SyncStrategy};
use fc_rpc::{EthTask, OverrideHandle};
use fc_rpc_core::types::{FeeHistoryCache, FeeHistoryCacheLimit, FilterPool};
use sc_network_common::sync::warp::WarpSyncParams;
use sc_network_sync::SyncingService;
use sc_rpc::SubscriptionTaskExecutor;

// Our native executor instance.
pub struct ExecutorDispatch;

impl sc_executor::NativeExecutionDispatch for ExecutorDispatch {
	/// Only enable the benchmarking host functions when we actually want to benchmark.
	#[cfg(feature = "runtime-benchmarks")]
	type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;
	/// Otherwise we only use the default Substrate host functions.
	#[cfg(not(feature = "runtime-benchmarks"))]
	type ExtendHostFunctions = ();

	fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
		societal_node_runtime::api::dispatch(method, data)
	}

	fn native_version() -> sc_executor::NativeVersion {
		societal_node_runtime::native_version()
	}
}

/// The full client type definition.
pub type FullClient =
	sc_service::TFullClient<Block, RuntimeApi, NativeElseWasmExecutor<ExecutorDispatch>>;
type FullBackend = sc_service::TFullBackend<Block>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;

#[cfg(feature = "manual-seal")]
pub type ConsensusResult = (FrontierBlockImport<Block, Arc<FullClient>, FullClient>, Sealing);

/// The transaction pool type definition.
pub type TransactionPool = sc_transaction_pool::FullPool<Block, FullClient>;

pub(crate) fn db_config_dir(config: &Configuration) -> PathBuf {
	config
		.base_path
		.as_ref()
		.map(|base_path| base_path.config_dir(config.chain_spec.id()))
		.unwrap_or_else(|| {
			BasePath::from_project("", "", &Cli::executable_name())
				.config_dir(config.chain_spec.id())
		})
}

pub fn new_partial(
	config: &Configuration,
	cli: &Cli,
	_force_dev: bool,
) -> Result<
	sc_service::PartialComponents<
		FullClient,
		FullBackend,
		FullSelectChain,
		sc_consensus::DefaultImportQueue<Block, FullClient>,
		sc_transaction_pool::FullPool<Block, FullClient>,
		(
			(
				FrontierBlockImport<
					Block,
					sc_consensus_grandpa::GrandpaBlockImport<
						FullBackend,
						Block,
						FullClient,
						FullSelectChain,
					>,
					FullClient,
				>,
				sc_consensus_grandpa::LinkHalf<Block, FullClient, FullSelectChain>,
			),
			sc_consensus_grandpa::SharedVoterState,
			Option<Telemetry>,
			Arc<FrontierBackend<Block>>,
			Option<FilterPool>,
			(FeeHistoryCache, FeeHistoryCacheLimit),
		),
	>,
	ServiceError,
> {
	if config.keystore_remote.is_some() {
		return Err(ServiceError::Other("Remote Keystores are not supported.".to_string()))
	}

	let telemetry = config
		.telemetry_endpoints
		.clone()
		.filter(|x| !x.is_empty())
		.map(|endpoints| -> Result<_, sc_telemetry::Error> {
			let worker = TelemetryWorker::new(16)?;
			let telemetry = worker.handle().new_telemetry(endpoints);
			Ok((worker, telemetry))
		})
		.transpose()?;

	let executor = NativeElseWasmExecutor::<ExecutorDispatch>::new(
		config.wasm_method,
		config.default_heap_pages,
		config.max_runtime_instances,
		config.runtime_cache_size,
	);

	let (client, backend, keystore_container, task_manager) =
		sc_service::new_full_parts::<Block, RuntimeApi, _>(
			config,
			telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
			executor,
		)?;
	let client = Arc::new(client);

	let telemetry = telemetry.map(|(worker, telemetry)| {
		task_manager.spawn_handle().spawn("telemetry", None, worker.run());
		telemetry
	});

	let select_chain = sc_consensus::LongestChain::new(backend.clone());

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		config.role.is_authority().into(),
		config.prometheus_registry(),
		task_manager.spawn_essential_handle(),
		client.clone(),
	);

	let frontier_backend = Arc::new(FrontierBackend::open(
		Arc::clone(&client),
		&config.database,
		&db_config_dir(config),
	)?);
	let filter_pool: Option<FilterPool> = Some(Arc::new(Mutex::new(BTreeMap::new())));
	let fee_history_cache: FeeHistoryCache = Arc::new(Mutex::new(BTreeMap::new()));
	let fee_history_cache_limit: FeeHistoryCacheLimit = cli.run.fee_history_limit;

	let (grandpa_block_import, grandpa_link) = sc_consensus_grandpa::block_import(
		client.clone(),
		&(client.clone() as Arc<_>),
		select_chain.clone(),
		telemetry.as_ref().map(|x| x.handle()),
	)?;
	let justification_import = grandpa_block_import.clone();

	let frontier_block_import =
		FrontierBlockImport::new(grandpa_block_import, client.clone(), frontier_backend.clone());

	let slot_duration = sc_consensus_aura::slot_duration(&*client)?;

	let import_queue =
		sc_consensus_aura::import_queue::<AuraPair, _, _, _, _, _>(ImportQueueParams {
			block_import: frontier_block_import.clone(),
			justification_import: Some(Box::new(justification_import)),
			client: client.clone(),
			create_inherent_data_providers: move |_, ()| async move {
				let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

				let slot =
					sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
						*timestamp,
						slot_duration,
					);

				Ok((slot, timestamp))
			},
			spawner: &task_manager.spawn_essential_handle(),
			registry: config.prometheus_registry(),
			check_for_equivocation: Default::default(),
			telemetry: telemetry.as_ref().map(|x| x.handle()),
			compatibility_mode: Default::default(),
		})?;

	let import_setup = (frontier_block_import, grandpa_link);

	Ok(sc_service::PartialComponents {
		client,
		backend,
		task_manager,
		keystore_container,
		select_chain,
		import_queue,
		transaction_pool,
		other: (
			import_setup,
			sc_consensus_grandpa::SharedVoterState::empty(),
			telemetry,
			frontier_backend,
			filter_pool,
			(fee_history_cache, fee_history_cache_limit),
		),
	})
}

/// Result of [`new_full_base`].
pub struct NewFullBase {
	/// The task manager of the node.
	pub task_manager: TaskManager,
	/// The client instance of the node.
	pub client: Arc<FullClient>,
	/// The networking service of the node.
	pub network: Arc<NetworkService<Block, <Block as BlockT>::Hash>>,
	/// The transaction pool of the node.
	pub transaction_pool: Arc<TransactionPool>,
	/// The rpc handlers of the node.
	pub rpc_handlers: RpcHandlers,
}

/// Creates a full service from the configuration.
pub fn new_full_base(
	mut config: Configuration,
	disable_hardware_benchmarks: bool,
	with_startup_data: impl FnOnce(
		&FrontierBlockImport<
			Block,
			sc_consensus_grandpa::GrandpaBlockImport<
				FullBackend,
				Block,
				FullClient,
				FullSelectChain,
			>,
			FullClient,
		>,
	),
	cli: &Cli,
) -> Result<NewFullBase, ServiceError> {
	let hwbench = if !disable_hardware_benchmarks {
		config.database.path().map(|database_path| {
			let _ = std::fs::create_dir_all(database_path);
			sc_sysinfo::gather_hwbench(Some(database_path))
		})
	} else {
		None
	};

	let sc_service::PartialComponents {
		client,
		backend,
		mut task_manager,
		import_queue,
		keystore_container,
		select_chain,
		transaction_pool,
		other:
			(
				import_setup,
				rpc_setup,
				mut telemetry,
				frontier_backend,
				filter_pool,
				(fee_history_cache, fee_history_cache_limit),
			),
	} = new_partial(&config, cli, false)?;

	let shared_voter_state = rpc_setup;
	let grandpa_protocol_name = sc_consensus_grandpa::protocol_standard_name(
		&client.block_hash(0).ok().flatten().expect("Genesis block exists; qed"),
		&config.chain_spec,
	);

	config
		.network
		.extra_sets
		.push(sc_consensus_grandpa::grandpa_peers_set_config(grandpa_protocol_name.clone()));

	let (_, grandpa_link) = &import_setup;

	config
		.network
		.extra_sets
		.push(sc_consensus_grandpa::grandpa_peers_set_config(grandpa_protocol_name.clone()));
	let warp_sync: Arc<dyn sc_network::config::WarpSyncProvider<Block>> =
		Arc::new(sc_consensus_grandpa::warp_proof::NetworkProvider::new(
			backend.clone(),
			grandpa_link.shared_authority_set().clone(),
			Vec::default(),
		));
	let warp_sync_params = Some(WarpSyncParams::WithProvider(warp_sync));

	let (network, system_rpc_tx, tx_handler_controller, network_starter, sync_service) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue,
			block_announce_validator_builder: None,
			warp_sync_params,
		})?;

	let client = client;
	let select_chain = select_chain;
	let keystore = keystore_container.sync_keystore();
	let prometheus_registry = config.prometheus_registry().cloned();
	let filter_pool = filter_pool;
	let frontier_backend = frontier_backend;
	let fee_history_cache = fee_history_cache;

	let overrides = crate::rpc::overrides_handle(client.clone());
	let block_data_cache = Arc::new(fc_rpc::EthBlockDataCacheTask::new(
		task_manager.spawn_handle(),
		overrides.clone(),
		50,
		50,
		prometheus_registry,
	));

	if config.offchain_worker.enabled {
		// TODO: use custom key here
		sp_keystore::SyncCryptoStore::sr25519_generate_new(
			&*keystore,
			societal_node_runtime::pallet_dao::KEY_TYPE,
			Some("//Alice"),
		)
		.expect("Creating key with account Alice should succeed.");

		sc_service::build_offchain_workers(
			&config,
			task_manager.spawn_handle(),
			client.clone(),
			network.clone(),
		);
	}

	let role = config.role.clone();
	let force_authoring = config.force_authoring;
	let backoff_authoring_blocks =
		Some(sc_consensus_slots::BackoffAuthoringOnFinalizedHeadLagging::default());
	let name = config.network.node_name.clone();
	let enable_grandpa = !config.disable_grandpa;
	let prometheus_registry = config.prometheus_registry().cloned();

	// TODO
	// Channel for the rpc handler to communicate with the authorship task.
	// let (command_sink, commands_stream) = mpsc::channel(1000);

	// Sinks for pubsub notifications.
	// Everytime a new subscription is created, a new mpsc channel is added to the sink pool.
	// The MappingSyncWorker sends through the channel on block import and the subscription emits a
	// notification to the subscriber on receiving a message through this channel. This way we avoid
	// race conditions when using native substrate block import notification stream.
	let pubsub_notification_sinks: fc_mapping_sync::EthereumBlockNotificationSinks<
		fc_mapping_sync::EthereumBlockNotification<Block>,
	> = Default::default();
	let pubsub_notification_sinks = Arc::new(pubsub_notification_sinks);

	let rpc_builder = {
		let client = client.clone();
		let pool = transaction_pool.clone();
		let is_authority = role.is_authority();
		let enable_dev_signer = cli.run.enable_dev_signer;
		let network = network.clone();
		let filter_pool = filter_pool.clone();
		let frontier_backend = frontier_backend.clone();
		let overrides = overrides.clone();
		let fee_history_cache = fee_history_cache.clone();
		let max_past_logs = cli.run.max_past_logs;
		let sync = sync_service.clone();
		let pubsub_notification_sinks = pubsub_notification_sinks.clone();

		move |deny_unsafe, subscription_executor: SubscriptionTaskExecutor| {
			let deps = crate::rpc::FullDeps {
				client: client.clone(),
				pool: pool.clone(),
				deny_unsafe,
				graph: pool.pool().clone(),
				is_authority,
				enable_dev_signer,
				network: network.clone(),
				sync: sync.clone(),
				filter_pool: filter_pool.clone(),
				frontier_backend: frontier_backend.clone(),
				max_past_logs,
				fee_history_cache: fee_history_cache.clone(),
				fee_history_cache_limit,
				xcm_senders: None,
				overrides: overrides.clone(),
				block_data_cache: block_data_cache.clone(),
			};

			crate::rpc::create_full(deps, subscription_executor, pubsub_notification_sinks.clone())
				.map_err(Into::into)
		}
	};

	let rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		config,
		backend: backend.clone(),
		client: client.clone(),
		keystore: keystore_container.sync_keystore(),
		network: network.clone(),
		rpc_builder: Box::new(rpc_builder),
		transaction_pool: transaction_pool.clone(),
		task_manager: &mut task_manager,
		system_rpc_tx,
		tx_handler_controller,
		sync_service: sync_service.clone(),
		telemetry: telemetry.as_mut(),
	})?;

	spawn_frontier_tasks(
		&task_manager,
		client.clone(),
		backend,
		frontier_backend,
		filter_pool,
		overrides,
		fee_history_cache,
		fee_history_cache_limit,
		sync_service.clone(),
		pubsub_notification_sinks,
	);

	if let Some(hwbench) = hwbench {
		sc_sysinfo::print_hwbench(&hwbench);

		if let Some(ref mut telemetry) = telemetry {
			let telemetry_handle = telemetry.handle();
			task_manager.spawn_handle().spawn(
				"telemetry_hwbench",
				None,
				sc_sysinfo::initialize_hwbench_telemetry(telemetry_handle, hwbench),
			);
		}
	}

	let (block_import, grandpa_link) = import_setup;

	// (with_startup_data)(&block_import, &babe_link);
	(with_startup_data)(&block_import);

	// Spawn authority discovery module.
	if role.is_authority() {
		let proposer_factory = sc_basic_authorship::ProposerFactory::new(
			task_manager.spawn_handle(),
			client.clone(),
			transaction_pool.clone(),
			prometheus_registry.as_ref(),
			telemetry.as_ref().map(|x| x.handle()),
		);

		let slot_duration = sc_consensus_aura::slot_duration(&*client)?;

		let aura = sc_consensus_aura::start_aura::<AuraPair, _, _, _, _, _, _, _, _, _, _>(
			StartAuraParams {
				slot_duration,
				client: client.clone(),
				select_chain,
				block_import,
				proposer_factory,
				create_inherent_data_providers: move |_, ()| async move {
					let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

					let slot =
						sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
							*timestamp,
							slot_duration,
						);

					Ok((slot, timestamp))
				},
				force_authoring,
				backoff_authoring_blocks,
				keystore: keystore_container.sync_keystore(),
				sync_oracle: sync_service.clone(),
				justification_sync_link: sync_service.clone(),
				block_proposal_slot_portion: SlotProportion::new(2f32 / 3f32),
				max_block_proposal_slot_portion: None,
				telemetry: telemetry.as_ref().map(|x| x.handle()),
				compatibility_mode: Default::default(),
			},
		)?;

		task_manager
			.spawn_essential_handle()
			.spawn_blocking("aura", Some("block-authoring"), aura);
	}

	// if the node isn't actively participating in consensus then it doesn't
	// need a keystore, regardless of which protocol we use below.
	let keystore =
		if role.is_authority() { Some(keystore_container.sync_keystore()) } else { None };

	let config = sc_consensus_grandpa::Config {
		// FIXME #1578 make this available through chainspec
		gossip_duration: std::time::Duration::from_millis(333),
		justification_period: 512,
		name: Some(name),
		observer_enabled: false,
		keystore,
		local_role: role,
		telemetry: telemetry.as_ref().map(|x| x.handle()),
		protocol_name: grandpa_protocol_name,
	};

	if enable_grandpa {
		// start the full GRANDPA voter
		// NOTE: non-authorities could run the GRANDPA observer protocol, but at
		// this point the full voter should provide better guarantees of block
		// and vote data availability than the observer. The observer has not
		// been tested extensively yet and having most nodes in a network run it
		// could lead to finality stalls.
		let grandpa_config = sc_consensus_grandpa::GrandpaParams {
			config,
			link: grandpa_link,
			network: network.clone(),
			telemetry: telemetry.as_ref().map(|x| x.handle()),
			voting_rule: sc_consensus_grandpa::VotingRulesBuilder::default().build(),
			prometheus_registry,
			shared_voter_state,
			sync: sync_service,
		};

		// the GRANDPA voter task is considered infallible, i.e.
		// if it fails we take down the service with it.
		task_manager.spawn_essential_handle().spawn_blocking(
			"grandpa-voter",
			None,
			sc_consensus_grandpa::run_grandpa_voter(grandpa_config)?,
		);
	}

	network_starter.start_network();
	Ok(NewFullBase { task_manager, client, network, transaction_pool, rpc_handlers })
}

/// Builds a new service for a full client.
pub fn new_full(
	config: Configuration,
	disable_hardware_benchmarks: bool,
	cli: &Cli,
) -> Result<TaskManager, ServiceError> {
	new_full_base(config, disable_hardware_benchmarks, |_| (), cli)
		.map(|NewFullBase { task_manager, .. }| task_manager)
}

fn spawn_frontier_tasks(
	task_manager: &TaskManager,
	client: Arc<FullClient>,
	backend: Arc<FullBackend>,
	frontier_backend: Arc<FrontierBackend<Block>>,
	filter_pool: Option<FilterPool>,
	overrides: Arc<OverrideHandle<Block>>,
	fee_history_cache: FeeHistoryCache,
	fee_history_cache_limit: FeeHistoryCacheLimit,
	sync: Arc<SyncingService<Block>>,
	pubsub_notification_sinks: Arc<
		fc_mapping_sync::EthereumBlockNotificationSinks<
			fc_mapping_sync::EthereumBlockNotification<Block>,
		>,
	>,
) {
	task_manager.spawn_essential_handle().spawn(
		"frontier-mapping-sync-worker",
		None,
		MappingSyncWorker::new(
			client.import_notification_stream(),
			Duration::new(6, 0),
			client.clone(),
			backend,
			overrides.clone(),
			frontier_backend,
			3,
			0,
			SyncStrategy::Normal,
			sync,
			pubsub_notification_sinks,
		)
		.for_each(|()| future::ready(())),
	);

	// Spawn Frontier EthFilterApi maintenance task.
	if let Some(filter_pool) = filter_pool {
		// Each filter is allowed to stay in the pool for 100 blocks.
		const FILTER_RETAIN_THRESHOLD: u64 = 100;
		task_manager.spawn_essential_handle().spawn(
			"frontier-filter-pool",
			None,
			EthTask::filter_pool_task(client.clone(), filter_pool, FILTER_RETAIN_THRESHOLD),
		);
	}

	// Spawn Frontier FeeHistory cache maintenance task.
	task_manager.spawn_essential_handle().spawn(
		"frontier-fee-history",
		None,
		EthTask::fee_history_task(client, overrides, fee_history_cache, fee_history_cache_limit),
	);
}
