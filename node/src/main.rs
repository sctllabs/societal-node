//! Societal Node CLI library.
#![warn(missing_docs)]

mod benchmarking;
mod chain_spec;
mod cli;
mod command;
#[cfg(any(feature = "parachain", feature = "runtime-benchmarks"))]
mod para_service;
mod rpc;
#[cfg(not(any(feature = "parachain", feature = "runtime-benchmarks")))]
mod service;

fn main() -> sc_cli::Result<()> {
	command::run()
}
