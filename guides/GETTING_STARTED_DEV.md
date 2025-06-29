## NEW DEV QUICKSTART GUIDE

New developer getting started working on the mrgnv2 program side? Read on.

### Things to Install (Feb 2025)

- rust/cargo - latest stable
- node - 23.0.0
- yarn - 1.22.22
- avm - 0.30.1
- anchor - 0.30.1
- solana - 1.18.17
- cargo-nextest - use `cargo install cargo-nextest --version "0.9.81" --locked` exactly
- cargo-fuzz - 0.12.0

## Running tests

### For unit tests:

```
cargo test --lib
```

### For the TS test suite:

```
anchor build -p marginfi -- --no-default-features
anchor test --skip-build
```

Note: you may need to build the other programs (mock, liquidity incentive, etc) if you have never run anchor build before.

Segmentation fault? Just try again. That happens sometimes.

### For the Rust test suite:

```
anchor build -p marginfi
./scripts/test-program.sh marginfi mainnet-beta
```

This is much slower than the remix test command, but stable on any system.

### Customize Your Rust testing experience:

```
./scripts/test-program-remix.sh -p marginfi -l warn -c mainnet-beta -f mainnet-beta
```

This will throttle your CPU and may error sporadically as a reminder to buy a better CPU if you try to do anything else (like say, compile another Rust repo) while this is running.

Benchmarks:

- 9700X: `Summary [   6.302s] 238 tests run: 238 passed, 0 skipped`

### To just one Rust test:

```
./scripts/single-test.sh marginfi accrue_interest --verbose
./scripts/single-test.sh test_name --verbose
```

## Common issues

### The TS suite fails with `Environment supports crypto:  false` at the top

Update Node

### All the tests are failing in Rust and/or TS

Make sure you build the correct version, Rust requires the mainnet version (default features), TS wants localnet (no features)
