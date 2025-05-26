use anchor_lang::{InstructionData, ToAccountMetas};
use fixtures::prelude::*;
use surroundfi::{constants::FEE_STATE_SEED, prelude::SurroundfiGroup};
use pretty_assertions::assert_eq;
use solana_program::{instruction::Instruction, system_program};
use solana_program_test::*;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction};

#[tokio::test]
async fn surroundfi_group_create_success() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    // Create & initialize surroundfi group
    let surroundfi_group_key = Keypair::new();

    let (fee_state_key, _bump) =
        Pubkey::find_program_address(&[FEE_STATE_SEED.as_bytes()], &surroundfi::id());

    let accounts = surroundfi::accounts::SurroundfiGroupInitialize {
        surroundfi_group: surroundfi_group_key.pubkey(),
        admin: test_f.payer(),
        fee_state: fee_state_key,
        system_program: system_program::id(),
    };
    let init_surroundfi_group_ix = Instruction {
        program_id: surroundfi::id(),
        accounts: accounts.to_account_metas(Some(true)),
        data: surroundfi::instruction::SurroundfiGroupInitialize {
            is_arena_group: false,
        }
        .data(),
    };
    let tx = Transaction::new_signed_with_payer(
        &[init_surroundfi_group_ix],
        Some(&test_f.payer().clone()),
        &[&test_f.payer_keypair(), &surroundfi_group_key],
        test_f.get_latest_blockhash().await,
    );
    let res = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(tx)
        .await;

    assert!(res.is_ok());

    // Fetch & deserialize surroundfi group account
    let surroundfi_group: SurroundfiGroup = test_f
        .load_and_deserialize(&surroundfi_group_key.pubkey())
        .await;

    // Check basic properties
    assert_eq!(surroundfi_group.admin, test_f.payer());
    // Program fees are always enabled by default (Note that mostly elsewhere in the test fixture,
    // we send a config to disable them, to simplify testing)
    assert_eq!(surroundfi_group.program_fees_enabled(), true);
    assert_eq!(surroundfi_group.is_arena_group(), false);

    Ok(())
}
