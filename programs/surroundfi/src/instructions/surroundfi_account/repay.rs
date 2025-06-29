use crate::{
    check,
    events::{AccountEventHeader, LendingAccountRepayEvent},
    prelude::{SurroundfiError, SurroundfiGroup, SurroundfiResult},
    state::{
        surroundfi_account::{BankAccountWrapper, SurroundfiAccount, ACCOUNT_DISABLED},
        surroundfi_group::Bank,
    },
    utils,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{TokenAccount, TokenInterface};
use fixed::types::I80F48;
use solana_program::{clock::Clock, sysvar::Sysvar};

/// 1. Accrue interest
/// 2. Find the user's existing bank account for the asset repaid
/// 3. Record liability decrease in the bank account
/// 4. Transfer funds from the signer's token account to the bank's liquidity vault
///
/// Will error if there is no existing liability <=> depositing is not allowed.
pub fn lending_account_repay<'info>(
    mut ctx: Context<'_, '_, 'info, 'info, LendingAccountRepay<'info>>,
    amount: u64,
    repay_all: Option<bool>,
) -> SurroundfiResult {
    let LendingAccountRepay {
        surroundfi_account: surroundfi_account_loader,
        authority: signer,
        signer_token_account,
        liquidity_vault: bank_liquidity_vault,
        token_program,
        bank: bank_loader,
        group: surroundfi_group_loader,
        ..
    } = ctx.accounts;
    let clock = Clock::get()?;
    let maybe_bank_mint = utils::maybe_take_bank_mint(
        &mut ctx.remaining_accounts,
        &*bank_loader.load()?,
        token_program.key,
    )?;

    let repay_all = repay_all.unwrap_or(false);
    let mut bank = bank_loader.load_mut()?;
    let mut surroundfi_account = surroundfi_account_loader.load_mut()?;

    check!(
        !surroundfi_account.get_flag(ACCOUNT_DISABLED),
        SurroundfiError::AccountDisabled
    );

    bank.accrue_interest(
        clock.unix_timestamp,
        &*surroundfi_group_loader.load()?,
        #[cfg(not(feature = "client"))]
        bank_loader.key(),
    )?;

    let mut bank_account = BankAccountWrapper::find(
        &bank_loader.key(),
        &mut bank,
        &mut surroundfi_account.lending_account,
    )?;

    let repay_amount_post_fee = if repay_all {
        bank_account.repay_all()?
    } else {
        bank_account.repay(I80F48::from_num(amount))?;

        amount
    };

    let repay_amount_pre_fee = maybe_bank_mint
        .as_ref()
        .map(|mint| {
            utils::calculate_pre_fee_spl_deposit_amount(
                mint.to_account_info(),
                repay_amount_post_fee,
                clock.epoch,
            )
        })
        .transpose()?
        .unwrap_or(repay_amount_post_fee);

    bank_account.deposit_spl_transfer(
        repay_amount_pre_fee,
        signer_token_account.to_account_info(),
        bank_liquidity_vault.to_account_info(),
        signer.to_account_info(),
        maybe_bank_mint.as_ref(),
        token_program.to_account_info(),
        ctx.remaining_accounts,
    )?;

    emit!(LendingAccountRepayEvent {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.authority.key()),
            surroundfi_account: surroundfi_account_loader.key(),
            surroundfi_account_authority: surroundfi_account.authority,
            surroundfi_group: surroundfi_account.group,
        },
        bank: bank_loader.key(),
        mint: bank.mint,
        amount: repay_amount_post_fee,
        close_balance: repay_all,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct LendingAccountRepay<'info> {
    pub group: AccountLoader<'info, SurroundfiGroup>,

    #[account(
        mut,
        has_one = group,
        has_one = authority
    )]
    pub surroundfi_account: AccountLoader<'info, SurroundfiAccount>,

    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = group,
        has_one = liquidity_vault
    )]
    pub bank: AccountLoader<'info, Bank>,

    /// CHECK: Token mint/authority are checked at transfer
    #[account(mut)]
    pub signer_token_account: AccountInfo<'info>,

    #[account(mut)]
    pub liquidity_vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Interface<'info, TokenInterface>,
}
