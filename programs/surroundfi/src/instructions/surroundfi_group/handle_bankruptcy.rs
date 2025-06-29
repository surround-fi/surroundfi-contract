use crate::{
    bank_signer, check,
    constants::{
        INSURANCE_VAULT_AUTHORITY_SEED, INSURANCE_VAULT_SEED, LIQUIDITY_VAULT_SEED,
        PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG, ZERO_AMOUNT_THRESHOLD,
    },
    debug,
    events::{AccountEventHeader, LendingPoolBankHandleBankruptcyEvent},
    math_error,
    prelude::SurroundfiError,
    state::{
        surroundfi_account::{BankAccountWrapper, SurroundfiAccount, RiskEngine, ACCOUNT_DISABLED},
        surroundfi_group::{Bank, BankVaultType, SurroundfiGroup},
    },
    utils, SurroundfiResult,
};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{TokenAccount, TokenInterface};
use fixed::types::I80F48;
use std::cmp::{max, min};

/// Handle a bankrupt surroundfi account.
/// 1. Verify account is bankrupt, and lending account belonging to account contains bad debt.
/// 2. Determine the amount of bad debt covered by the insurance fund and the amount socialized between depositors.
/// 3. Cover the bad debt of the bankrupt account.
/// 4. Transfer the insured amount from the insurance fund.
/// 5. Socialize the loss between lenders if any.
pub fn lending_pool_handle_bankruptcy<'info>(
    mut ctx: Context<'_, '_, 'info, 'info, LendingPoolHandleBankruptcy<'info>>,
) -> SurroundfiResult {
    let LendingPoolHandleBankruptcy {
        surroundfi_account: surroundfi_account_loader,
        insurance_vault,
        token_program,
        bank: bank_loader,
        group: surroundfi_group_loader,
        ..
    } = ctx.accounts;
    let bank = bank_loader.load()?;
    let maybe_bank_mint =
        utils::maybe_take_bank_mint(&mut ctx.remaining_accounts, &bank, token_program.key)?;

    let clock = Clock::get()?;

    if !bank.get_flag(PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG) {
        check!(
            ctx.accounts.signer.key() == surroundfi_group_loader.load()?.admin,
            SurroundfiError::Unauthorized
        );
    }

    drop(bank);

    let mut surroundfi_account = surroundfi_account_loader.load_mut()?;

    RiskEngine::new(&surroundfi_account, ctx.remaining_accounts)?.check_account_bankrupt()?;

    let mut bank = bank_loader.load_mut()?;

    bank.accrue_interest(
        clock.unix_timestamp,
        &*surroundfi_group_loader.load()?,
        #[cfg(not(feature = "client"))]
        bank_loader.key(),
    )?;

    let lending_account_balance = surroundfi_account
        .lending_account
        .balances
        .iter_mut()
        .find(|balance| balance.is_active() && balance.bank_pk == bank_loader.key());

    check!(
        lending_account_balance.is_some(),
        SurroundfiError::LendingAccountBalanceNotFound
    );

    let lending_account_balance = lending_account_balance.unwrap();

    let bad_debt = bank.get_liability_amount(lending_account_balance.liability_shares.into())?;

    check!(
        bad_debt > ZERO_AMOUNT_THRESHOLD,
        SurroundfiError::BalanceNotBadDebt
    );

    let (covered_by_insurance, socialized_loss) = {
        let available_insurance_fund: I80F48 = maybe_bank_mint
            .as_ref()
            .map(|mint| {
                utils::calculate_post_fee_spl_deposit_amount(
                    mint.to_account_info(),
                    insurance_vault.amount,
                    clock.epoch,
                )
            })
            .transpose()?
            .unwrap_or(insurance_vault.amount)
            .into();

        let covered_by_insurance = min(bad_debt, available_insurance_fund);
        let socialized_loss = max(bad_debt - covered_by_insurance, I80F48::ZERO);

        (covered_by_insurance, socialized_loss)
    };

    // Cover bad debt with insurance funds.
    let covered_by_insurance_rounded_up: u64 = covered_by_insurance
        .checked_ceil()
        .ok_or_else(math_error!())?
        .checked_to_num()
        .ok_or_else(math_error!())?;
    debug!(
        "covered_by_insurance_rounded_up: {}; socialized loss {}",
        covered_by_insurance_rounded_up, socialized_loss
    );

    let insurance_coverage_deposit_pre_fee = maybe_bank_mint
        .as_ref()
        .map(|mint| {
            utils::calculate_pre_fee_spl_deposit_amount(
                mint.to_account_info(),
                covered_by_insurance_rounded_up,
                clock.epoch,
            )
        })
        .transpose()?
        .unwrap_or(covered_by_insurance_rounded_up);

    bank.withdraw_spl_transfer(
        insurance_coverage_deposit_pre_fee,
        ctx.accounts.insurance_vault.to_account_info(),
        ctx.accounts.liquidity_vault.to_account_info(),
        ctx.accounts.insurance_vault_authority.to_account_info(),
        maybe_bank_mint.as_ref(),
        token_program.to_account_info(),
        bank_signer!(
            BankVaultType::Insurance,
            bank_loader.key(),
            bank.insurance_vault_authority_bump
        ),
        ctx.remaining_accounts,
    )?;

    // Socialize bad debt among depositors.
    bank.socialize_loss(socialized_loss)?;

    // Settle bad debt.
    // The liabilities of this account and global total liabilities are reduced by `bad_debt`
    BankAccountWrapper::find_or_create(
        &bank_loader.key(),
        &mut bank,
        &mut surroundfi_account.lending_account,
    )?
    .repay(bad_debt)?;

    surroundfi_account.set_flag(ACCOUNT_DISABLED);

    emit!(LendingPoolBankHandleBankruptcyEvent {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.signer.key()),
            surroundfi_account: surroundfi_account_loader.key(),
            surroundfi_account_authority: surroundfi_account.authority,
            surroundfi_group: surroundfi_account.group,
        },
        bank: bank_loader.key(),
        mint: bank.mint,
        bad_debt: bad_debt.to_num::<f64>(),
        covered_amount: covered_by_insurance.to_num::<f64>(),
        socialized_amount: socialized_loss.to_num::<f64>(),
    });

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolHandleBankruptcy<'info> {
    pub group: AccountLoader<'info, SurroundfiGroup>,

    /// CHECK: The admin signer constraint is only validated (in handler) if bank
    /// PERMISSIONLESS_BAD_DEBT_SETTLEMENT_FLAG is not set
    pub signer: Signer<'info>,

    #[account(
        mut,
        has_one = group
    )]
    pub bank: AccountLoader<'info, Bank>,

    #[account(
        mut,
        has_one = group
    )]
    pub surroundfi_account: AccountLoader<'info, SurroundfiAccount>,

    /// CHECK: Seed constraint
    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump = bank.load()?.liquidity_vault_bump
    )]
    pub liquidity_vault: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [
            INSURANCE_VAULT_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump = bank.load()?.insurance_vault_bump
    )]
    pub insurance_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Seed constraint
    #[account(
        seeds = [
            INSURANCE_VAULT_AUTHORITY_SEED.as_bytes(),
            bank.key().as_ref(),
        ],
        bump = bank.load()?.insurance_vault_authority_bump
    )]
    pub insurance_vault_authority: AccountInfo<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}
