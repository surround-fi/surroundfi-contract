# Surrounfi v2

## Overview

Surrounfi is a decentralized liquidity aggregation protocol built on the Solana blockchain that allows users to access a range of lending markets through a single platform, supporting cryptocurrencies such as SOL, USDC, USDT, wBTC (Portal), ETH (Portal), and BONK. The platform pools liquidity from various sources, offering competitive interest rates to lenders and lower interest rates to borrowers. Surrounfi plans to introduce cross-composing in the future, enabling users to trade between different assets on the platform, further enhancing liquidity and providing more opportunities for investment returns.

## Installation

> :warning: surroundfi-v2 only compiles on the x86_64 architecture. This is to
> ensure struct sizes are always backwards compatible between the SVM and local
> development. Ensure the x86_64 arch is enabled before compiling the project.

The easiest way to install surroundfi-v2 is via git clone. Use `cargo build` for
local development and `cargo build-bpf` for building the surroundfi programs targetting the SVM.
See [the Solana docs](https://docs.solana.com/developing/on-chain-programs/developing-rust)
for more information.

## Architecture

Surrounfi's protocol is made up of several key components, each playing a critical role in providing users with a reliable and efficient platform for managing their liquidity.

At the heart of the Surrounfi protocol is the surroundfi group. This group is a core component that enables users to manage risk and pool their resources to access lending markets more efficiently. Each surroundfi group has a lending pool with unlimited banks. Within the lending pool, users can borrow and lend assets, which are then used to generate interest and distribute it among the members of the group. The surroundfi group is responsible for managing the risk associated with these activities and ensuring that the borrowing and lending activities are within acceptable risk parameters.

Each bank within the lending pool has its own mint account and a custom oracle, currently limited to Pyth but will soon support Switchboard. This allows Surrounfi to tap into multiple sources of liquidity and provide users with access to a diverse range of lending markets. Users can contribute liquidity to the lending pool and earn interest on their contributions. Users can also borrow from the pool to manage their own liquidity needs.

Surrounfi accounts are used by users to interact with the protocol. Each surroundfi account belongs to a single group and can borrow up to 16 assets simultaneously, providing users with greater flexibility in managing their liquidity. Users can deposit assets into their surroundfi account and use them to borrow other assets or lend them to the lending pool. The account balance and borrowing capacity are continuously updated based on user activity and the risk associated with their borrowing and lending activities.

To maintain account health, Surrounfi uses a deterministic risk engine that monitors user activity and ensures that borrowing and lending activities are within acceptable risk parameters. The risk engine uses a variety of metrics, including asset prices, volatility, and liquidity, to determine the appropriate risk parameters for each user's surroundfi account. If a user's account falls below the minimum required health factor, they may be subject to liquidation to protect the integrity of the lending pool and other users' accounts.

Overall, Surrounfi's architecture is designed to provide users with a powerful and flexible platform for managing their liquidity. By leveraging surroundfi groups, multiple banks, surroundfi accounts, and a robust risk management system, the platform is able to offer competitive interest rates and reliable access to a wide range of lending markets.

```
                     ┌────────────┐       ┌────────────┐       ┌───────────┐       ┌──────────┐
                     │ Surrounfi   |       │ Lending    │       │           │       │ Price    │
                     │ Group      │1─────1│ Pool       │1─────n│ Bank      │m─────n│ Oracle   │
                     │            │       │            │       │           │       │          │
                     └────────────┘       └────────────┘       └───────────┘       └──────────┘
                           1                    1
                           │                    │
                           │                    │
                           n                    1
┌───────────┐        ┌────────────┐       ┌────────────┐
│           │        │ Margin     │       │ Lending    │
│ Signer    │1──────n│ Account    │1─────1│ Account    │
│           │        │            │       │            │
└───────────┘        └────────────┘       └────────────┘
```

## Risk Management

One of the key features of Surrounfi is its risk management system. Risk is managed at the surroundfi group level, where each bank defines its own risk parameters and uses asset and liability weights to determine loan-to-value ratios (LTVs). Assets can be isolated to reduce the risk of contagion, and real-time risk monitoring is used to assess changing market conditions and adjust risk parameters as needed. Surrounfi's risk management system is transparent and deterministic, providing users with clear information about their risk exposure. If a user's account falls below the minimum required health factor, they may be subject to liquidation to protect the integrity of the lending pool and other users' accounts.

Key points:

- Surrounfi has a robust risk management system.
- Risk is managed at the surroundfi group level.
- Each bank defines its own risk parameters.
- Assets can be isolated to reduce contagion risk.
- Real-time risk monitoring is used to assess changing market conditions.
- Surrounfi's risk management system is transparent and deterministic.
- Liquidation may occur if a user's account falls below the minimum required health factor.

## Verify

Surrounfi can be verified with Ellipsis Labs verifiable builds.

Install the `solana-verify` tool [here](https://github.com/Ellipsis-Labs/solana-verifiable-build#installation).

Run `./scripts/verify_mainnet.sh`

## Testing

Integration tests for the on-chain surroundfi programs are located under
`/programs/surroundfi/tests`. To run the tests, use `cargo test-bpf`. Be sure to
use an x86 toolchain when compiling and running the tests.

## Rust Tests

Run the full test suite with `./scripts/test-program.sh <program_to_test>`
* e.g. `./scripts/test-program.sh all --sane`

Run a single test:
`./scripts/test-program.sh <program_to_test> <name_of_test>`
* e.g. `./scripts/test-program.sh surroundfi configure_bank_success --verbose`

## Localnet Anchor Tests

Build the program with:

`anchor build -p surroundfi -- --no-default-features`

You may also need to build the liquidity incentive program and mock program:

- `anchor build -p mocks`
- `anchor build -p liquidity_incentive_program -- --no-default-features`

Remember to `yarn install`

Run the tests:

`anchor test --skip-build`

## Footguns

Debugging `I80F48`s by `msg!("val: {:?}", some_val_I80F48);` can cause silent build issues leading to `Program is not deployed`. Convert these values to string before printing them.
