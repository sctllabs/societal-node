use std::net::SocketAddr;

use crate::service;
use cumulus_client_cli::generate_genesis_block;
use cumulus_primitives_core::ParaId;
use fc_db::DatabaseSource;
use frame_benchmarking_cli::{BenchmarkCmd, ExtrinsicFactory, SUBSTRATE_REFERENCE_HARDWARE};
use log::info;
use sc_cli::{
	ChainSpec, CliConfiguration, DefaultConfigurationValues, ImportParams, KeystoreParams,
	NetworkParams, Result, RuntimeVersion, SharedParams, SubstrateCli,
};
use sc_service::{config::PrometheusConfig, BasePath, PartialComponents};
use service::{new_partial, ExecutorDispatch};
use sp_core::{hexdisplay::HexDisplay, Encode};
use sp_runtime::traits::{AccountIdConversion, Block as BlockT};

use societal_node_runtime::{Block, EXISTENTIAL_DEPOSIT};
use sp_keyring::Sr25519Keyring;

use crate::{
	chain_spec,
	cli::{Cli, RelayChainCli, Subcommand},
};

use crate::benchmarking::{inherent_benchmark_data, RemarkBuilder, TransferKeepAliveBuilder};

fn load_spec(id: &str) -> std::result::Result<Box<dyn ChainSpec>, String> {
	Ok(match id {
		"dev" => Box::new(chain_spec::development_config()),
		"template-rococo" => Box::new(chain_spec::local_testnet_config()),
		"" | "local" => Box::new(chain_spec::local_testnet_config()),
		path => Box::new(chain_spec::ChainSpec::from_json_file(std::path::PathBuf::from(path))?),
	})
}

impl SubstrateCli for Cli {
	fn impl_name() -> String {
		"Societal Node".into()
	}

	fn impl_version() -> String {
		env!("SUBSTRATE_CLI_IMPL_VERSION").into()
	}

	fn description() -> String {
		env!("CARGO_PKG_DESCRIPTION").into()
	}

	fn author() -> String {
		env!("CARGO_PKG_AUTHORS").into()
	}

	fn support_url() -> String {
		"support.anonymous.an".into()
	}

	fn copyright_start_year() -> i32 {
		2017
	}

	fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
		load_spec(id)
	}

	fn native_runtime_version(_: &Box<dyn ChainSpec>) -> &'static RuntimeVersion {
		&societal_node_runtime::VERSION
	}
}

impl SubstrateCli for RelayChainCli {
	fn impl_name() -> String {
		"Societal Parachain Collator".into()
	}

	fn impl_version() -> String {
		env!("SUBSTRATE_CLI_IMPL_VERSION").into()
	}

	fn description() -> String {
		env!("CARGO_PKG_DESCRIPTION").into()
	}

	fn author() -> String {
		env!("CARGO_PKG_AUTHORS").into()
	}

	fn support_url() -> String {
		"support.anonymous.an".into()
	}

	fn copyright_start_year() -> i32 {
		2017
	}

	fn load_spec(&self, id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
		polkadot_cli::Cli::from_iter([RelayChainCli::executable_name()].iter()).load_spec(id)
	}

	fn native_runtime_version(chain_spec: &Box<dyn ChainSpec>) -> &'static RuntimeVersion {
		polkadot_cli::Cli::native_runtime_version(chain_spec)
	}
}

impl DefaultConfigurationValues for RelayChainCli {
	fn p2p_listen_port() -> u16 {
		30334
	}

	fn rpc_ws_listen_port() -> u16 {
		9945
	}

	fn rpc_http_listen_port() -> u16 {
		9934
	}

	fn prometheus_listen_port() -> u16 {
		9616
	}
}

impl CliConfiguration<Self> for RelayChainCli {
	fn shared_params(&self) -> &SharedParams {
		self.base.base.shared_params()
	}

	fn import_params(&self) -> Option<&ImportParams> {
		self.base.base.import_params()
	}

	fn network_params(&self) -> Option<&NetworkParams> {
		self.base.base.network_params()
	}

	fn keystore_params(&self) -> Option<&KeystoreParams> {
		self.base.base.keystore_params()
	}

	fn base_path(&self) -> Result<Option<BasePath>> {
		Ok(self
			.shared_params()
			.base_path()?
			.or_else(|| self.base_path.clone().map(Into::into)))
	}

	fn rpc_http(&self, default_listen_port: u16) -> Result<Option<SocketAddr>> {
		self.base.base.rpc_http(default_listen_port)
	}

	fn rpc_ipc(&self) -> Result<Option<String>> {
		self.base.base.rpc_ipc()
	}

	fn rpc_ws(&self, default_listen_port: u16) -> Result<Option<SocketAddr>> {
		self.base.base.rpc_ws(default_listen_port)
	}

	fn prometheus_config(
		&self,
		default_listen_port: u16,
		chain_spec: &Box<dyn ChainSpec>,
	) -> Result<Option<PrometheusConfig>> {
		self.base.base.prometheus_config(default_listen_port, chain_spec)
	}

	fn init<F>(
		&self,
		_support_url: &String,
		_impl_version: &String,
		_logger_hook: F,
		_config: &sc_service::Configuration,
	) -> Result<()>
	where
		F: FnOnce(&mut sc_cli::LoggerBuilder, &sc_service::Configuration),
	{
		unreachable!("PolkadotCli is never initialized; qed");
	}

	fn chain_id(&self, is_dev: bool) -> Result<String> {
		let chain_id = self.base.base.chain_id(is_dev)?;

		Ok(if chain_id.is_empty() { self.chain_id.clone().unwrap_or_default() } else { chain_id })
	}

	fn role(&self, is_dev: bool) -> Result<sc_service::Role> {
		self.base.base.role(is_dev)
	}

	fn transaction_pool(&self, is_dev: bool) -> Result<sc_service::config::TransactionPoolOptions> {
		self.base.base.transaction_pool(is_dev)
	}

	fn trie_cache_maximum_size(&self) -> Result<Option<usize>> {
		self.base.base.trie_cache_maximum_size()
	}

	fn rpc_methods(&self) -> Result<sc_service::config::RpcMethods> {
		self.base.base.rpc_methods()
	}

	fn rpc_ws_max_connections(&self) -> Result<Option<usize>> {
		self.base.base.rpc_ws_max_connections()
	}

	fn rpc_cors(&self, is_dev: bool) -> Result<Option<Vec<String>>> {
		self.base.base.rpc_cors(is_dev)
	}

	fn default_heap_pages(&self) -> Result<Option<u64>> {
		self.base.base.default_heap_pages()
	}

	fn force_authoring(&self) -> Result<bool> {
		self.base.base.force_authoring()
	}

	fn disable_grandpa(&self) -> Result<bool> {
		self.base.base.disable_grandpa()
	}

	fn max_runtime_instances(&self) -> Result<Option<usize>> {
		self.base.base.max_runtime_instances()
	}

	fn announce_block(&self) -> Result<bool> {
		self.base.base.announce_block()
	}

	fn telemetry_endpoints(
		&self,
		chain_spec: &Box<dyn ChainSpec>,
	) -> Result<Option<sc_telemetry::TelemetryEndpoints>> {
		self.base.base.telemetry_endpoints(chain_spec)
	}

	fn node_name(&self) -> Result<String> {
		self.base.base.node_name()
	}
}

/// Parse command line arguments into service configuration.
pub fn run() -> sc_cli::Result<()> {
	let cli = Cli::from_args();

	match &cli.subcommand {
		Some(Subcommand::Key(cmd)) => cmd.run(&cli),
		Some(Subcommand::BuildSpec(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
		},
		Some(Subcommand::CheckBlock(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, import_queue, .. } =
					service::new_partial(&config, &cli, false)?;
				Ok((cmd.run(client, import_queue), task_manager))
			})
		},
		Some(Subcommand::ExportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, .. } =
					service::new_partial(&config, &cli, false)?;
				Ok((cmd.run(client, config.database), task_manager))
			})
		},
		Some(Subcommand::ExportState(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, .. } =
					service::new_partial(&config, &cli, false)?;
				Ok((cmd.run(client, config.chain_spec), task_manager))
			})
		},
		Some(Subcommand::ImportBlocks(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, import_queue, .. } =
					service::new_partial(&config, &cli, false)?;
				Ok((cmd.run(client, import_queue), task_manager))
			})
		},
		Some(Subcommand::PurgeChain(cmd)) => {
			let runner = cli.create_runner(cmd)?;

			runner.sync_run(|config| {
				// Although the cumulus_client_cli::PurgeCommand will extract the relay chain id,
				// we need to extract it here to determine whether we are running the dev service.
				let extension = chain_spec::Extensions::try_get(&*config.chain_spec);
				let relay_chain_id = extension.map(|e| e.relay_chain.clone());
				let dev_service =
					cli.run.dev_service || relay_chain_id == Some("dev-service".to_string());

				// Remove Frontier offchain db
				let frontier_database_config = match config.database {
					DatabaseSource::RocksDb { .. } => DatabaseSource::RocksDb {
						path: service::frontier_database_dir(&config, "db"),
						cache_size: 0,
					},
					DatabaseSource::ParityDb { .. } => DatabaseSource::ParityDb {
						path: service::frontier_database_dir(&config, "paritydb"),
					},
					_ =>
						return Err(format!("Cannot purge `{:?}` database", config.database).into()),
				};
				cmd.base.run(frontier_database_config)?;

				if dev_service {
					// base refers to the encapsulated "regular" sc_cli::PurgeChain command
					return cmd.base.run(config.database)
				}

				let polkadot_cli = RelayChainCli::new(
					&config,
					[RelayChainCli::executable_name()].iter().chain(cli.relay_chain_args.iter()),
				);

				let polkadot_config = SubstrateCli::create_configuration(
					&polkadot_cli,
					&polkadot_cli,
					config.tokio_handle.clone(),
				)
				.map_err(|err| format!("Relay chain argument error: {err}"))?;

				cmd.run(config, polkadot_config)
			})
		},
		Some(Subcommand::Revert(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				let PartialComponents { client, task_manager, backend, .. } =
					new_partial(&config, &cli, false)?;
				Ok((cmd.run(client, backend, None), task_manager))
			})
		},
		Some(Subcommand::ExportGenesisState(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|_config| {
				let spec = cli.load_spec(&cmd.shared_params.chain.clone().unwrap_or_default())?;
				let state_version = Cli::native_runtime_version(&spec).state_version();
				cmd.run::<Block>(&*spec, state_version)
			})
		},
		Some(Subcommand::ExportGenesisWasm(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|_config| {
				let spec = cli.load_spec(&cmd.shared_params.chain.clone().unwrap_or_default())?;
				cmd.run(&*spec)
			})
		},
		Some(Subcommand::Benchmark(cmd)) => {
			let runner = cli.create_runner(cmd)?;

			runner.sync_run(|config| {
				// This switch needs to be in the client, since the client decides
				// which sub-commands it wants to support.
				match cmd {
					BenchmarkCmd::Pallet(cmd) => {
						if !cfg!(feature = "runtime-benchmarks") {
							return Err(
								"Runtime benchmarking wasn't enabled when building the node. \
							You can enable it with `--features runtime-benchmarks`."
									.into(),
							)
						}

						cmd.run::<Block, ExecutorDispatch>(config)
					},
					BenchmarkCmd::Block(cmd) => {
						// ensure that we keep the task manager alive
						let PartialComponents { client, .. } =
							service::new_partial(&config, &cli, false)?;
						cmd.run(client)
					},
					#[cfg(not(feature = "runtime-benchmarks"))]
					BenchmarkCmd::Storage(_) => Err(
						"Storage benchmarking can be enabled with `--features runtime-benchmarks`."
							.into(),
					),
					#[cfg(feature = "runtime-benchmarks")]
					BenchmarkCmd::Storage(cmd) => {
						let PartialComponents { client, backend, .. } =
							service::new_partial(&config, &cli, false)?;
						let db = backend.expose_db();
						let storage = backend.expose_storage();

						cmd.run(config, client, db, storage)
					},
					BenchmarkCmd::Overhead(cmd) => {
						// ensure that we keep the task manager alive
						let PartialComponents { client, .. } =
							service::new_partial(&config, &cli, false)?;
						let ext_builder = RemarkBuilder::new(client.clone());

						cmd.run(
							config,
							client,
							inherent_benchmark_data()?,
							Vec::new(),
							&ext_builder,
						)
					},
					BenchmarkCmd::Extrinsic(cmd) => {
						// ensure that we keep the task manager alive
						let partial = service::new_partial(&config, &cli, false)?;
						// Register the *Remark* and *TKA* builders.
						let ext_factory = ExtrinsicFactory(vec![
							Box::new(RemarkBuilder::new(partial.client.clone())),
							Box::new(TransferKeepAliveBuilder::new(
								partial.client.clone(),
								Sr25519Keyring::Alice.to_account_id(),
								EXISTENTIAL_DEPOSIT,
							)),
						]);

						cmd.run(
							partial.client,
							inherent_benchmark_data()?,
							Vec::new(),
							&ext_factory,
						)
					},
					BenchmarkCmd::Machine(cmd) =>
						cmd.run(&config, SUBSTRATE_REFERENCE_HARDWARE.clone()),
				}
			})
		},
		#[cfg(feature = "try-runtime")]
		Some(Subcommand::TryRuntime(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.async_run(|config| {
				// we don't need any of the components of new_partial, just a runtime, or a task
				// manager to do `async_run`.
				let registry = config.prometheus_config.as_ref().map(|cfg| &cfg.registry);
				let task_manager =
					sc_service::TaskManager::new(config.tokio_handle.clone(), registry)
						.map_err(|e| sc_cli::Error::Service(sc_service::Error::Prometheus(e)))?;

				Ok((cmd.run::<Block, ExecutorDispatch>(config), task_manager))
			})
		},
		None => {
			let runner = cli.create_runner(&(*cli.run).normalize())?;

			runner.run_node_until_exit(|config| async move {
				let collator_options = cli.run.collator_options();

				let hwbench = if !cli.no_hardware_benchmarks {
					config.database.path().map(|database_path| {
						let _ = std::fs::create_dir_all(database_path);
						sc_sysinfo::gather_hwbench(Some(database_path))
					})
				} else {
					None
				};

				let extension = chain_spec::Extensions::try_get(&*config.chain_spec);
				let para_id = extension.map(|e| e.para_id);
				let id = ParaId::from(cli.run.parachain_id.or(para_id).unwrap_or(1000));

				// If dev service was requested, start up manual or instant seal.
				// Otherwise continue with the normal parachain node.
				// Dev service can be requested in two ways.
				// 1. by providing the --dev-service flag to the CLI
				// 2. by specifying "dev-service" in the chain spec's "relay-chain" field.
				// NOTE: the --dev flag triggers the dev service by way of number 2
				let relay_chain_id = extension.map(|e| e.relay_chain.clone());
				let dev_service =
					cli.run.dev_service || relay_chain_id == Some("dev-service".to_string());

				if dev_service {
					return service::new_dev(config, cli.run.sealing, &cli, hwbench)
						.map_err(Into::into)
				}

				let polkadot_cli = RelayChainCli::new(
					&config,
					[RelayChainCli::executable_name()].iter().chain(cli.relay_chain_args.iter()),
				);

				let parachain_account =
					AccountIdConversion::<polkadot_primitives::v2::AccountId>::into_account_truncating(&id);

				let state_version =
					RelayChainCli::native_runtime_version(&config.chain_spec).state_version();

				let block: Block = generate_genesis_block(&*config.chain_spec, state_version)?;
				let genesis_state = format!("0x{:?}", HexDisplay::from(&block.header().encode()));

				let tokio_handle = config.tokio_handle.clone();
				let polkadot_config =
					SubstrateCli::create_configuration(&polkadot_cli, &polkadot_cli, tokio_handle)
						.map_err(|err| format!("Relay chain argument error: {err}"))?;

				info!("Parachain id: {:?}", id);
				info!("Parachain Account: {}", parachain_account);
				info!("Parachain genesis state: {}", genesis_state);

				service::start_parachain_node(
					config,
					polkadot_config,
					collator_options,
					id,
					&cli,
					hwbench,
				)
				.await
				.map(|r| r.0)
				.map_err(Into::into)
			})
		},
	}
}
