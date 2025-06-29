use crate::{
    config::Config,
    utils::{find_fee_state_pda, process_transaction, ui_to_native},
};
use anchor_client::anchor_lang::{prelude::*, InstructionData};
use anchor_spl::associated_token;
use anyhow::Result;
use surroundfi::{
    bank_authority_seed,
    state::surroundfi_group::{Bank, BankVaultType},
};
use solana_sdk::{
    instruction::Instruction, message::Message, pubkey::Pubkey, transaction::Transaction,
};

pub fn process_collect_fees(config: Config, bank_pk: Pubkey, fee_ata: Pubkey) -> Result<()> {
    let bank = config.sfi_program.account::<Bank>(bank_pk)?;
    let rpc_client = config.sfi_program.rpc();

    let (liquidity_vault_authority, _) = Pubkey::find_program_address(
        bank_authority_seed!(BankVaultType::Liquidity, bank_pk),
        &surroundfi::id(),
    );

    let mut ix = Instruction {
        program_id: surroundfi::id(),
        accounts: surroundfi::accounts::LendingPoolCollectBankFees {
            group: bank.group,
            bank: bank_pk,
            fee_vault: bank.fee_vault,
            token_program: spl_token::id(),
            liquidity_vault_authority,
            liquidity_vault: bank.liquidity_vault,
            insurance_vault: bank.insurance_vault,
            fee_state: find_fee_state_pda(&surroundfi::id()).0,
            fee_ata,
        }
        .to_account_metas(Some(true)),
        data: surroundfi::instruction::LendingPoolCollectBankFees {}.data(),
    };
    ix.accounts
        .push(AccountMeta::new_readonly(bank.mint, false));

    let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
    let signing_keypairs = config.get_signers(false);

    let message = Message::new(&[ix], Some(&config.authority()));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.partial_sign(&signing_keypairs, recent_blockhash);

    match process_transaction(&transaction, &rpc_client, config.get_tx_mode()) {
        Ok(sig) => println!("Tx succeded (sig: {})", sig),
        Err(err) => println!("Error:\n{:#?}", err),
    };

    Ok(())
}

pub fn process_withdraw_fees(
    config: Config,
    bank_pk: Pubkey,
    amount_ui: f64,
    dst_address: Option<Pubkey>,
) -> Result<()> {
    let bank = config.sfi_program.account::<Bank>(bank_pk)?;
    let amount = ui_to_native(amount_ui, bank.mint_decimals);
    let dst_address = dst_address.unwrap_or(config.authority());
    let ata = associated_token::get_associated_token_address(&dst_address, &bank.mint);

    let rpc_client = config.sfi_program.rpc();

    let (fee_vault_authority, _) = Pubkey::find_program_address(
        bank_authority_seed!(BankVaultType::Fee, bank_pk),
        &surroundfi::id(),
    );

    let create_ata_ix =
        spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &config.authority(),
            &config.authority(),
            &bank.mint,
            &spl_token::id(),
        );

    let mut ix = Instruction {
        program_id: surroundfi::id(),
        accounts: surroundfi::accounts::LendingPoolWithdrawFees {
            group: bank.group,
            bank: bank_pk,
            admin: config.authority(),
            fee_vault: bank.fee_vault,
            fee_vault_authority,
            dst_token_account: ata,
            token_program: spl_token::id(),
        }
        .to_account_metas(Some(true)),
        data: surroundfi::instruction::LendingPoolWithdrawFees { amount }.data(),
    };
    ix.accounts
        .push(AccountMeta::new_readonly(bank.mint, false));

    let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
    let signing_keypairs = config.get_signers(false);

    let message = Message::new(&[create_ata_ix, ix], Some(&config.authority()));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.partial_sign(&signing_keypairs, recent_blockhash);

    match process_transaction(&transaction, &rpc_client, config.get_tx_mode()) {
        Ok(sig) => println!("Tx succeded (sig: {})", sig),
        Err(err) => println!("Error:\n{:#?}", err),
    };

    Ok(())
}

pub fn process_withdraw_insurance(
    config: Config,
    bank_pk: Pubkey,
    amount_ui: f64,
    dst_address: Option<Pubkey>,
) -> Result<()> {
    let bank = config.sfi_program.account::<Bank>(bank_pk)?;
    let amount = ui_to_native(amount_ui, bank.mint_decimals);
    let dst_address = dst_address.unwrap_or(config.authority());
    let ata = associated_token::get_associated_token_address(&dst_address, &bank.mint);

    let rpc_client = config.sfi_program.rpc();

    let (insurance_vault_authority, _) = Pubkey::find_program_address(
        bank_authority_seed!(BankVaultType::Insurance, bank_pk),
        &surroundfi::id(),
    );

    let create_ata_ix =
        spl_associated_token_account::instruction::create_associated_token_account_idempotent(
            &config.authority(),
            &config.authority(),
            &bank.mint,
            &spl_token::id(),
        );

    let mut ix = Instruction {
        program_id: surroundfi::id(),
        accounts: surroundfi::accounts::LendingPoolWithdrawInsurance {
            group: bank.group,
            bank: bank_pk,
            admin: config.authority(),
            insurance_vault: bank.insurance_vault,
            insurance_vault_authority,
            dst_token_account: ata,
            token_program: spl_token::id(),
        }
        .to_account_metas(Some(true)),
        data: surroundfi::instruction::LendingPoolWithdrawInsurance { amount }.data(),
    };
    ix.accounts
        .push(AccountMeta::new_readonly(bank.mint, false));

    let recent_blockhash = rpc_client.get_latest_blockhash().unwrap();
    let signing_keypairs = config.get_signers(false);

    let message = Message::new(&[create_ata_ix, ix], Some(&config.authority()));
    let mut transaction = Transaction::new_unsigned(message);
    transaction.partial_sign(&signing_keypairs, recent_blockhash);

    match process_transaction(&transaction, &rpc_client, config.get_tx_mode()) {
        Ok(sig) => println!("Tx succeded (sig: {})", sig),
        Err(err) => println!("Error:\n{:#?}", err),
    };

    Ok(())
}
