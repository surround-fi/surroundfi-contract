Surroundfi Deployment Notes - Mainnet

### Requirements

- Install [Anchor](https://www.anchor-lang.com/docs/installation)
- A wallet with SOL fund available as keypair in `~/.config/solana/id.json`

### Deploy to mainnet

1. Clone & move to SurroundFi program directory

2. In `anchor.toml`: update cluster to Sonic SVM Mainnet RPC `https://api.mainnet-beta.sonic.game/`

3. Build & update program IDs
    - Build program: `anchor build -p surroundfi`
    - Get program keys: `anchor keys list`
      ->  Then update `declare_id!` in `programs/<program-name>/src/lib.rs` with respective program key from terminal.
    - Build again: `anchor build -p surroundfi`

4. Deploy program: `anchor deploy -p surroundfi`

### Close program

SOL fund used when we deployed program will be refunded after closing the program

Sequentially run these commands:

```shell
solana program close ./target/deploy/surroundfi-keypair.json

# bypass warning & confirm to close program
solana program close ./target/deploy/surroundfi-keypair.json --bypass-warning
```

### Configure SurroundFi program

1. Init fee state
2. Init group
3. Add banks
4. Configure oracle for banks
5. …
