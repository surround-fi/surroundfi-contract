use fixed_macro::types::I80F48;
use fixtures::{assert_custom_error, prelude::*};
use surroundfi::{
    prelude::SurroundfiError,
    state::surroundfi_group::{BankConfig, BankConfigOpt, BankOperationalState},
};
use pretty_assertions::assert_eq;
use solana_program_test::*;

#[tokio::test]
async fn surroundfi_group_bank_paused_should_error() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            config: None,
        }],
        protocol_fees: false,
    }))
    .await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    test_f
        .surroundfi_group
        .try_lending_pool_configure_bank(
            usdc_bank_f,
            BankConfigOpt {
                operational_state: Some(BankOperationalState::Paused),
                ..BankConfigOpt::default()
            },
        )
        .await?;

    let lender_mfi_account_f = test_f.create_surroundfi_account().await;
    let lender_token_account_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    let res = lender_mfi_account_f
        .try_bank_deposit(lender_token_account_usdc.key, usdc_bank_f, 100_000, None)
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), SurroundfiError::BankPaused);

    Ok(())
}

#[tokio::test]
async fn surroundfi_group_bank_reduce_only_withdraw_success() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            config: None,
        }],
        protocol_fees: false,
    }))
    .await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    let lender_mfi_account_f = test_f.create_surroundfi_account().await;
    let lender_token_account_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender_mfi_account_f
        .try_bank_deposit(lender_token_account_usdc.key, usdc_bank_f, 100_000, None)
        .await?;

    usdc_bank_f
        .update_config(
            BankConfigOpt {
                operational_state: Some(BankOperationalState::ReduceOnly),
                ..Default::default()
            },
            None,
        )
        .await?;

    let res = lender_mfi_account_f
        .try_bank_withdraw(lender_token_account_usdc.key, usdc_bank_f, 0, Some(true))
        .await;

    assert!(res.is_ok());

    Ok(())
}

#[tokio::test]
async fn surroundfi_group_bank_reduce_only_deposit_success() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: None,
            },
            TestBankSetting {
                mint: BankMint::Sol,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    ..*DEFAULT_SOL_TEST_BANK_CONFIG
                }),
            },
        ],
        protocol_fees: false,
    }))
    .await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    let lender_1_mfi_account = test_f.create_surroundfi_account().await;
    let lender_1_token_account_sol = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    lender_1_mfi_account
        .try_bank_deposit(lender_1_token_account_sol.key, sol_bank_f, 100, None)
        .await?;

    let lender_2_mfi_account = test_f.create_surroundfi_account().await;
    let lender_2_token_account_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    lender_2_mfi_account
        .try_bank_deposit(lender_2_token_account_usdc.key, usdc_bank_f, 100_000, None)
        .await?;

    let lender_2_token_account_sol = test_f.sol_mint.create_empty_token_account().await;
    lender_2_mfi_account
        .try_bank_borrow(lender_2_token_account_sol.key, sol_bank_f, 1)
        .await?;

    usdc_bank_f
        .update_config(
            BankConfigOpt {
                operational_state: Some(BankOperationalState::ReduceOnly),
                ..Default::default()
            },
            None,
        )
        .await?;

    let res = lender_2_mfi_account
        .try_bank_repay(lender_2_token_account_sol.key, sol_bank_f, 1, None)
        .await;

    assert!(res.is_ok());

    Ok(())
}

#[tokio::test]
async fn surroundfi_group_bank_reduce_only_borrow_failure() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![
            TestBankSetting {
                mint: BankMint::Usdc,
                config: None,
            },
            TestBankSetting {
                mint: BankMint::Sol,
                config: Some(BankConfig {
                    asset_weight_init: I80F48!(1).into(),
                    ..*DEFAULT_SOL_TEST_BANK_CONFIG
                }),
            },
        ],
        protocol_fees: false,
    }))
    .await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);
    let sol_bank_f = test_f.get_bank(&BankMint::Sol);

    let lender_mfi_account = test_f.create_surroundfi_account().await;
    let lender_token_account_sol = test_f.sol_mint.create_token_account_and_mint_to(100).await;
    lender_mfi_account
        .try_bank_deposit(lender_token_account_sol.key, sol_bank_f, 100, None)
        .await?;

    let borrower_mfi_account = test_f.create_surroundfi_account().await;
    let borrower_token_account_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;
    borrower_mfi_account
        .try_bank_deposit(borrower_token_account_usdc.key, usdc_bank_f, 100_000, None)
        .await?;

    sol_bank_f
        .update_config(
            BankConfigOpt {
                operational_state: Some(BankOperationalState::ReduceOnly),
                ..Default::default()
            },
            None,
        )
        .await?;

    let borrower_token_account_sol = test_f.sol_mint.create_empty_token_account().await;
    let res = borrower_mfi_account
        .try_bank_borrow(borrower_token_account_sol.key, sol_bank_f, 1)
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), SurroundfiError::BankReduceOnly);

    Ok(())
}

#[tokio::test]
async fn surroundfi_group_bank_reduce_only_deposit_failure() -> anyhow::Result<()> {
    let test_f = TestFixture::new(Some(TestSettings {
        banks: vec![TestBankSetting {
            mint: BankMint::Usdc,
            config: None,
        }],
        protocol_fees: false,
    }))
    .await;

    let usdc_bank_f = test_f.get_bank(&BankMint::Usdc);

    usdc_bank_f
        .update_config(
            BankConfigOpt {
                operational_state: Some(BankOperationalState::ReduceOnly),
                ..Default::default()
            },
            None,
        )
        .await?;

    let lender_mfi_account_f = test_f.create_surroundfi_account().await;
    let lender_token_account_usdc = test_f
        .usdc_mint
        .create_token_account_and_mint_to(100_000)
        .await;

    let res = lender_mfi_account_f
        .try_bank_deposit(lender_token_account_usdc.key, usdc_bank_f, 100_000, None)
        .await;

    assert!(res.is_err());
    assert_custom_error!(res.unwrap_err(), SurroundfiError::BankReduceOnly);

    Ok(())
}
