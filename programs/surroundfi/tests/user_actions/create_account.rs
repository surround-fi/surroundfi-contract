use anchor_lang::{InstructionData, ToAccountMetas};
use fixtures::test::TestFixture;
use surroundfi::state::surroundfi_account::SurroundfiAccount;
use solana_program_test::tokio;
use solana_sdk::{
    instruction::Instruction, signature::Keypair, signer::Signer, system_program,
    transaction::Transaction,
};

#[tokio::test]
async fn surroundfi_account_create_success() -> anyhow::Result<()> {
    let test_f = TestFixture::new(None).await;

    let surroundfi_account_key = Keypair::new();
    let accounts = surroundfi::accounts::SurroundfiAccountInitialize {
        surroundfi_group: test_f.surroundfi_group.key,
        surroundfi_account: surroundfi_account_key.pubkey(),
        authority: test_f.payer(),
        fee_payer: test_f.payer(),
        system_program: system_program::id(),
    };
    let init_surroundfi_account_ix = Instruction {
        program_id: surroundfi::id(),
        accounts: accounts.to_account_metas(Some(true)),
        data: surroundfi::instruction::SurroundfiAccountInitialize {}.data(),
    };

    let tx = Transaction::new_signed_with_payer(
        &[init_surroundfi_account_ix],
        Some(&test_f.payer()),
        &[&test_f.payer_keypair(), &surroundfi_account_key],
        test_f.get_latest_blockhash().await,
    );

    let res = test_f
        .context
        .borrow_mut()
        .banks_client
        .process_transaction(tx)
        .await;

    assert!(res.is_ok());

    let surroundfi_account: SurroundfiAccount = test_f
        .load_and_deserialize(&surroundfi_account_key.pubkey())
        .await;

    assert_eq!(surroundfi_account.group, test_f.surroundfi_group.key);
    assert_eq!(surroundfi_account.authority, test_f.payer());
    assert!(surroundfi_account
        .lending_account
        .balances
        .iter()
        .all(|bank| !bank.is_active()));

    Ok(())
}