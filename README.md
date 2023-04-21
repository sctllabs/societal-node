# Societal Node

## Getting Started

Follow the steps below to get started with the Societal's node.

### Using Nix

Install [nix](https://nixos.org/) and optionally [direnv](https://github.com/direnv/direnv) and
[lorri](https://github.com/nix-community/lorri) for a fully plug and play experience for setting up
the development environment. To get all the correct dependencies activate direnv `direnv allow` and
lorri `lorri shell`.

### Rust Setup

First, complete the [basic Rust setup instructions](./docs/rust-setup.md).

### Run

Use Rust's native `cargo` command to build and launch the node:

```sh
cargo run --release -- --dev
```

### Build(Solo chain mode)

The `cargo run` command will perform an initial build. Use the following command to build the node
without launching it:

```sh
cargo build --release
```

### Build(Parachain mode)

The `cargo run` command will perform an initial build. Use the following command to build the node
without launching it:

```sh
cargo build --release --features=parachain
```

### Embedded Docs

Once the project has been built, the following command can be used to explore all parameters and
subcommands:

```sh
./target/release/societal-node -h
```

## Run

The provided `cargo run` command will launch a temporary node and its state will be discarded after
you terminate the process. After the project has been built, there are other ways to launch the
node.

### Single-Node Development Chain

This command will start the single-node development chain with non-persistent state:

```bash
./target/release/societal-node --dev --enable-offchain-indexing true
```

Purge the development chain's state:

```bash
./target/release/societal-node purge-chain --dev
```

Start the development chain with detailed logging:

```bash
RUST_BACKTRACE=1 ./target/release/societal-node -ldebug --dev --enable-offchain-indexing true
```

> Development chain means that the state of our chain will be in a tmp folder while the nodes are
> running. Also, **alice** account will be authority and sudo account as declared in the
> [genesis state](https://github.com/sctllabs/societal-node/blob/main/node/src/chain_spec.rs#L49).
> At the same time the following accounts will be pre-funded:
> - Alice
> - Bob
> - Alice//stash
> - Bob//stash

In case of being interested in maintaining the chain' state between runs a base path must be added
so the db can be stored in the provided folder instead of a temporary one. We could use this folder
to store different chain databases, as a different folder will be created for each chain that
is ran. The following commands shows how to use a newly created folder as our db base path.

```bash
// Create a folder to use as the db base path
$ mkdir my-chain-state

// Use of that folder to store the chain state
$ ./target/release/societal-node --dev --base-path ./my-chain-state/

// Check the folder structure created inside the base path after running the chain
$ ls ./my-chain-state
chains
$ ls ./my-chain-state/chains/
dev
$ ls ./my-chain-state/chains/dev
db keystore network
```

### Connect with Polkadot-JS Apps Front-end

Once the node template is running locally, you can connect it with **Polkadot-JS Apps** front-end
to interact with your chain. [Click
here](https://polkadot.js.org/apps/#/explorer?rpc=ws://localhost:9944) connecting the Apps to your
local node template.

### Connect with Societal Front-end

Alternativley you can use the **Societal's front-end** to connect with the local node. Please find the societal front-end repo [here](https://github.com/sctllabs/societal-front-end). Follow the readme for instructions on how to build and run the front-end. 

### Societal's Runtime

Review Societal's [runtime implementation](./runtime/src/lib.rs) included in this node. This file configures several pallets that make up Societal's runtime. Each pallet has its own code block that begins with `impl $PALLET_NAME::Config for Runtime`, which define its configuration settings. Then the pallets are composed into a single runtime by way of the `construct_runtime!` macro. 

### Run in Docker

First, install [Docker](https://docs.docker.com/get-docker/) and
[Docker Compose](https://docs.docker.com/compose/install/).

Then run the following command to start a single node development chain.

```bash
./scripts/docker_run.sh
```

This command will firstly compile your code, and then start a local development network. You can
also replace the default command
(`cargo build --release && ./target/release/societal-node --dev --ws-external`)
by appending your own. A few useful ones are as follow.

```bash
# Run Soceital node without re-compiling
./scripts/docker_run.sh ./target/release/societal-node --dev --ws-external --enable-offchain-indexing true

# Purge the local dev chain
./scripts/docker_run.sh ./target/release/societal-node purge-chain --dev

# Check whether the code is compilable
./scripts/docker_run.sh cargo check
```

### Run Rococo Local Testnet with Societal Node Parachains
```bash
# Run Rococo local testnet docker-compose configuration
./scripts/rococo_testnet_docker_run.sh
```

### Unit Test

To run Unit Tests, execute the following command:

```bash
cargo test
```
