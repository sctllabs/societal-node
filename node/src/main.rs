//! Societal Node CLI library.
#![warn(missing_docs)]

mod chain_spec;
mod service;
#[cfg(any(feature = "parachain", feature = "runtime-benchmarks"))]
mod para_service;
mod benchmarking;
mod cli;
mod command;
mod rpc;

fn main() -> sc_cli::Result<()> {
	command::run()
}
