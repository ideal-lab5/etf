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

### Testing

**Unit Tests**

``` sh
cargo test
```

**E2E Tests**

``` sh
cargo test --features e2e
```