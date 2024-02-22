# Encryption to the Future

This repository contains implementations of the ETF consensus mechanism and a substrate node that uses it.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="./resources/web3%20foundation_grants_badge_white.png">
  <img alt="This project is funded by the Web3 Foundation Grants Program" src="./resources/web3%20foundation_grants_badge_black.png">
</picture>


### Build

Use the following command to build the node without launching it:

```sh
cargo build --release
```

**Docker**

From the root directory, run:

``` sh
docker build .
```

### Run

To run a node as a validator, you first need to generate a new sr25519 keypair and insert it into the node. The guide [here](https://docs.substrate.io/tutorials/build-a-blockchain/add-trusted-nodes/) has detailed information on generating a key. 

After building the codebase in release mode, run:

``` sh
./target/release/node key generate --scheme Sr25519 --password-interactive
```

This outputs a randomly generated sr25519 keypair that we will use for signing payloads with offchain workers.  

Insert the key into your local keystore with the following command. Make sure to modify the base path to an appropriate directory. Also note that the key-type is `etfn`. The chain spec can either be a fresh chain spec locally generated, or else download the `etfDevSpecRaw` chainspec included in the main branch of this repo.

``` sh
./target/release/node key insert --base-path /tmp/node01 \
  --chain customSpecRaw.json \
  --scheme Sr25519 \
  --suri <your-secret-seed> \
  --password-interactive \
  --key-type etfn
```

### Testing

**Unit Tests**

``` sh
cargo test
```

**E2E Tests**

``` sh
cargo test --features e2e
```

**Benchmarks**

Build with benchmarks using:
``` sh
cargo build --release --features runtime-benchmarks
```

and run them with:
``` 
# list all benchmarks
./target/release/node benchmark pallet --chain dev --pallet "*" --extrinsic "*" --repeat 0
# benchmark the etf pallet
./target/release/node benchmark pallet \
    --chain dev \
    --wasm-execution=compiled \
    --pallet pallet_etf \
    --extrinsic "*" \
    --steps 50 \
    --repeat 20 \
    --output /pallets/etf/src/weight.rs
```

## CLI

``` sh
Key utilities for the cli

Usage: node etf <COMMAND>

Commands:
  generate  Generate a random node key, write it to a file or stdout and write the corresponding peer-id to stderr
  inspect   output the public key to a file or stdout 
  init      The `init` command used to build shares for an initial committee (ACSS.Reshare)
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```