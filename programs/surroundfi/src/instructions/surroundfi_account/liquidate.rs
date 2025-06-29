use crate::constants::{
    INSURANCE_VAULT_SEED, LIQUIDATION_INSURANCE_FEE, LIQUIDATION_LIQUIDATOR_FEE,
};
use crate::events::{AccountEventHeader, LendingAccountLiquidateEvent, LiquidationBalances};
use crate::state::surroundfi_account::{
    calc_amount, calc_value, get_remaining_accounts_per_bank, RiskEngine,
};
use crate::state::surroundfi_group::{Bank, BankVaultType};
use crate::state::price::{OraclePriceFeedAdapter, OraclePriceType, PriceAdapter, PriceBias};
use crate::utils::{validate_asset_tags, validate_bank_asset_tags};
use crate::{
    bank_signer,
    constants::{LIQUIDITY_VAULT_AUTHORITY_SEED, LIQUIDITY_VAULT_SEED},
    state::surroundfi_account::{BankAccountWrapper, SurroundfiAccount},
};
use crate::{check, debug, prelude::*, utils};
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{TokenAccount, TokenInterface};
use fixed::types::I80F48;
use solana_program::clock::Clock;
use solana_program::sysvar::Sysvar;

/// Instruction liquidates a position owned by a margin account that is in a unhealthy state.
/// The liquidator can purchase discounted collateral from the unhealthy account, in exchange for paying its debt.
///
/// ### Liquidation math:
/// #### Symbol definitions:
/// - `L`: Liability token
/// - `A`: Asset token
/// - `q_ll`: Quantity of `L` paid by the liquidator
/// - `q_lf`: Quantity of `L` received by the liquidatee
/// - `q_a`: Quantity of `A` to be liquidated
/// - `p_l`: Price of `L`
/// - `p_a`: Price of `A`
/// - `f_l`: Liquidation fee
/// - `f_i`: Insurance fee
///
/// The liquidator invokes this instruction with `q_a` as input (the total amount of collateral to be liquidated).
/// This is done because `q_a` is the most bounded variable in this process, as if the `q_a` is larger than what the liquidatee has, the instruction will fail.
/// The liquidator can observe how much collateral the liquidatee has, and ensures that the liquidatee will have enough collateral regardless of price action.
///
/// Fees:
/// The liquidator fee is charged in the conversion between the market value of the collateral being liquidated and the liability being covered by the liquidator.
/// The value of the liability is discounted by the liquidation fee.
///
/// The insurance fee is taken from the difference between liability being paid by the liquidator and the liability being received by the liquidatee.
/// This difference is deposited into the insurance fund.
///
/// Accounting changes in the liquidation process:
/// 1. The liquidator removes `q_ll` of `L`
/// 2. The liquidatee receives `q_lf` of `L`
/// 3. The liquidatee removes `q_a` of `A`
/// 4. The liquidator receives `q_a` of `A`
/// 5. The insurance fund receives `q_ll - q_lf` of `L`
///
/// Calculations:
///  
/// `q_ll = q_a * p_a * (1 - f_l) / p_l`
/// `q_lf = q_a * p_a * (1 - (f_l + f_i)) / p_l`
///
/// Risk model
///
/// Assumptions:
///
/// The fundamental idea behind the risk model is that the liquidation always leaves the liquidatee account in a better health than before.
/// This can be achieved by ensuring that for each token the liability has a LTV > 1, each collateral has a risk haircut of < 1,
/// with this by paying down the liability with the collateral, the liquidatee account will always be in a better health,
/// assuming that the liquidatee liability token balance doesn't become positive (doesn't become counted as collateral),
/// and that the liquidatee collateral token balance doesn't become negative (doesn't become counted as liability).
///
///
/// Expected remaining account schema
/// [
///    liab_mint_ai (if token2022 mint),
///    asset_oracle_ai,
///    liab_oracle_ai,
///    liquidator_observation_ais...,
///    liquidatee_observation_ais...,
///  ]

pub fn lending_account_liquidate<'info>(
    mut ctx: Context<'_, '_, 'info, 'info, LendingAccountLiquidate<'info>>,
    asset_amount: u64,
) -> SurroundfiResult {
    check!(asset_amount > 0, SurroundfiError::ZeroLiquidationAmount);

    check!(
        ctx.accounts.asset_bank.key() != ctx.accounts.liab_bank.key(),
        SurroundfiError::SameAssetAndLiabilityBanks
    );

    // Liquidators must repay debts in allowed asset types. A SOL debt can be repaid in any asset. A
    // Staked Collateral debt must be repaid in SOL or staked collateral. A Default asset debt can
    // be repaid in any Default asset or SOL.
    {
        let asset_bank = ctx.accounts.asset_bank.load()?;
        let liab_bank = ctx.accounts.liab_bank.load()?;
        validate_bank_asset_tags(&asset_bank, &liab_bank)?;

        // Sanity check user/liquidator accounts will not contain positions with mismatching tags
        // after liquidation.
        // * Note: user will be repaid in liab_bank
        let user_acc = ctx.accounts.liquidatee_surroundfi_account.load()?;
        validate_asset_tags(&liab_bank, &user_acc)?;
        // * Note: Liquidator repays liab bank, and is paid in asset_bank.
        let liquidator_acc = ctx.accounts.liquidator_surroundfi_account.load()?;
        validate_asset_tags(&liab_bank, &liquidator_acc)?;
        validate_asset_tags(&asset_bank, &liquidator_acc)?;
    } // release immutable borrow of asset_bank/liab_bank + liquidatee/liquidator user accounts

    let LendingAccountLiquidate {
        liquidator_surroundfi_account: liquidator_surroundfi_account_loader,
        liquidatee_surroundfi_account: liquidatee_surroundfi_account_loader,
        group: surroundfi_group_loader,
        ..
    } = ctx.accounts;

    let mut liquidator_surroundfi_account = liquidator_surroundfi_account_loader.load_mut()?;
    let mut liquidatee_surroundfi_account = liquidatee_surroundfi_account_loader.load_mut()?;
    let clock = Clock::get()?;
    let current_timestamp = clock.unix_timestamp;

    let maybe_liab_bank_mint = utils::maybe_take_bank_mint(
        &mut ctx.remaining_accounts,
        &*ctx.accounts.liab_bank.load()?,
        ctx.accounts.token_program.key,
    )?;
    {
        let group = &*surroundfi_group_loader.load()?;
        ctx.accounts.asset_bank.load_mut()?.accrue_interest(
            current_timestamp,
            group,
            #[cfg(not(feature = "client"))]
            ctx.accounts.asset_bank.key(),
        )?;
        ctx.accounts.liab_bank.load_mut()?.accrue_interest(
            current_timestamp,
            group,
            #[cfg(not(feature = "client"))]
            ctx.accounts.liab_bank.key(),
        )?;
    }

    let init_liquidatee_remaining_len = liquidatee_surroundfi_account.get_remaining_accounts_len()?;

    let liquidatee_accounts_starting_pos =
        ctx.remaining_accounts.len() - init_liquidatee_remaining_len;
    let liquidatee_remaining_accounts = &ctx.remaining_accounts[liquidatee_accounts_starting_pos..];

    let pre_liquidation_health =
        RiskEngine::new(&liquidatee_surroundfi_account, liquidatee_remaining_accounts)?
            .check_pre_liquidation_condition_and_get_account_health(
                &ctx.accounts.liab_bank.key(),
            )?;

    // ##Accounting changes##

    let (pre_balances, post_balances) = {
        let asset_amount = I80F48::from_num(asset_amount);

        let mut asset_bank = ctx.accounts.asset_bank.load_mut()?;
        let asset_bank_remaining_accounts_len = get_remaining_accounts_per_bank(&asset_bank)? - 1;

        let asset_price = {
            let oracle_ais = &ctx.remaining_accounts[0..asset_bank_remaining_accounts_len];
            let asset_pf = OraclePriceFeedAdapter::try_from_bank_config(
                &asset_bank.config,
                oracle_ais,
                &clock,
            )?;
            asset_pf.get_price_of_type(OraclePriceType::RealTime, Some(PriceBias::Low))?
        };

        let mut liab_bank = ctx.accounts.liab_bank.load_mut()?;
        let liab_bank_remaining_accounts_len = get_remaining_accounts_per_bank(&liab_bank)? - 1;
        let liab_price = {
            let oracle_ais = &ctx.remaining_accounts[asset_bank_remaining_accounts_len
                ..(asset_bank_remaining_accounts_len + liab_bank_remaining_accounts_len)];
            let liab_pf = OraclePriceFeedAdapter::try_from_bank_config(
                &liab_bank.config,
                oracle_ais,
                &clock,
            )?;
            liab_pf.get_price_of_type(OraclePriceType::RealTime, Some(PriceBias::High))?
        };

        let final_discount = I80F48::ONE - (LIQUIDATION_INSURANCE_FEE + LIQUIDATION_LIQUIDATOR_FEE);
        let liquidator_discount = I80F48::ONE - LIQUIDATION_LIQUIDATOR_FEE;

        // Quantity of liability to be paid off by liquidator
        let liab_amount_liquidator = calc_amount(
            calc_value(
                asset_amount,
                asset_price,
                asset_bank.mint_decimals,
                Some(liquidator_discount),
            )?,
            liab_price,
            liab_bank.mint_decimals,
        )?;

        // Quantity of liability to be received by liquidatee
        let liab_amount_final = calc_amount(
            calc_value(
                asset_amount,
                asset_price,
                asset_bank.mint_decimals,
                Some(final_discount),
            )?,
            liab_price,
            liab_bank.mint_decimals,
        )?;

        // Insurance fund fee
        let insurance_fund_fee = liab_amount_liquidator - liab_amount_final;

        assert!(
            insurance_fund_fee >= I80F48::ZERO,
            "Insurance fund fee cannot be negative"
        );

        debug!(
            "liab_quantity_liq: {}, liab_q_final: {}, asset_amount: {}, insurance_fund_fee: {}",
            liab_amount_liquidator, liab_amount_final, asset_amount, insurance_fund_fee
        );

        // Liquidator pays off liability
        let (liquidator_liability_pre_balance, liquidator_liability_post_balance) = {
            let mut bank_account = BankAccountWrapper::find_or_create(
                &ctx.accounts.liab_bank.key(),
                &mut liab_bank,
                &mut liquidator_surroundfi_account.lending_account,
            )?;

            let pre_balance = bank_account
                .bank
                .get_liability_amount(bank_account.balance.liability_shares.into())?;

            bank_account.decrease_balance_in_liquidation(liab_amount_liquidator)?;

            let post_balance = bank_account
                .bank
                .get_liability_amount(bank_account.balance.liability_shares.into())?;

            (pre_balance, post_balance)
        };

        // Liquidatee pays off `asset_quantity` amount of collateral
        let (liquidatee_asset_pre_balance, liquidatee_asset_post_balance) = {
            let mut bank_account = BankAccountWrapper::find(
                &ctx.accounts.asset_bank.key(),
                &mut asset_bank,
                &mut liquidatee_surroundfi_account.lending_account,
            )?;

            let pre_balance = bank_account
                .bank
                .get_asset_amount(bank_account.balance.asset_shares.into())?;

            bank_account
                .withdraw(asset_amount)
                .map_err(|_| SurroundfiError::OverliquidationAttempt)?;

            let post_balance = bank_account
                .bank
                .get_asset_amount(bank_account.balance.asset_shares.into())?;

            (pre_balance, post_balance)
        };

        // Liquidator receives `asset_quantity` amount of collateral
        let (liquidator_asset_pre_balance, liquidator_asset_post_balance) = {
            let mut bank_account = BankAccountWrapper::find_or_create(
                &ctx.accounts.asset_bank.key(),
                &mut asset_bank,
                &mut liquidator_surroundfi_account.lending_account,
            )?;

            let pre_balance = bank_account
                .bank
                .get_asset_amount(bank_account.balance.asset_shares.into())?;

            bank_account.increase_balance_in_liquidation(asset_amount)?;

            let post_balance = bank_account
                .bank
                .get_asset_amount(bank_account.balance.asset_shares.into())?;

            (pre_balance, post_balance)
        };

        let (insurance_fee_to_transfer, insurance_fee_dust) = (
            insurance_fund_fee
                .checked_to_num::<u64>()
                .ok_or(SurroundfiError::MathError)?,
            insurance_fund_fee.frac(),
        );

        let (liquidatee_liability_pre_balance, liquidatee_liability_post_balance) = {
            // Liquidatee receives liability payment
            let liab_bank_liquidity_authority_bump = liab_bank.liquidity_vault_authority_bump;

            let mut liquidatee_liab_bank_account = BankAccountWrapper::find_or_create(
                &ctx.accounts.liab_bank.key(),
                &mut liab_bank,
                &mut liquidatee_surroundfi_account.lending_account,
            )?;

            let liquidatee_liability_pre_balance =
                liquidatee_liab_bank_account.bank.get_liability_amount(
                    liquidatee_liab_bank_account.balance.liability_shares.into(),
                )?;

            liquidatee_liab_bank_account.increase_balance(liab_amount_final)?;

            let liquidatee_liability_post_balance =
                liquidatee_liab_bank_account.bank.get_liability_amount(
                    liquidatee_liab_bank_account.balance.liability_shares.into(),
                )?;

            // ## SPL transfer ##
            // Insurance fund receives fee
            liquidatee_liab_bank_account.withdraw_spl_transfer(
                insurance_fee_to_transfer,
                ctx.accounts.bank_liquidity_vault.to_account_info(),
                ctx.accounts.bank_insurance_vault.to_account_info(),
                ctx.accounts
                    .bank_liquidity_vault_authority
                    .to_account_info(),
                maybe_liab_bank_mint.as_ref(),
                ctx.accounts.token_program.to_account_info(),
                bank_signer!(
                    BankVaultType::Liquidity,
                    ctx.accounts.liab_bank.key(),
                    liab_bank_liquidity_authority_bump
                ),
                ctx.remaining_accounts,
            )?;

            (
                liquidatee_liability_pre_balance,
                liquidatee_liability_post_balance,
            )
        };

        liab_bank.collected_insurance_fees_outstanding =
            I80F48::from(liab_bank.collected_insurance_fees_outstanding)
                .checked_add(insurance_fee_dust)
                .ok_or(SurroundfiError::MathError)?
                .into();

        (
            LiquidationBalances {
                liquidatee_asset_balance: liquidatee_asset_pre_balance.to_num::<f64>(),
                liquidatee_liability_balance: liquidatee_liability_pre_balance.to_num::<f64>(),
                liquidator_asset_balance: liquidator_asset_pre_balance.to_num::<f64>(),
                liquidator_liability_balance: liquidator_liability_pre_balance.to_num::<f64>(),
            },
            LiquidationBalances {
                liquidatee_asset_balance: liquidatee_asset_post_balance.to_num::<f64>(),
                liquidatee_liability_balance: liquidatee_liability_post_balance.to_num::<f64>(),
                liquidator_asset_balance: liquidator_asset_post_balance.to_num::<f64>(),
                liquidator_liability_balance: liquidator_liability_post_balance.to_num::<f64>(),
            },
        )
    };

    // ## Risk checks ##

    let liquidator_remaining_acc_len = liquidator_surroundfi_account.get_remaining_accounts_len()?;
    let liquidator_accounts_starting_pos =
        liquidatee_accounts_starting_pos - liquidator_remaining_acc_len;

    let liquidator_remaining_accounts =
        &ctx.remaining_accounts[liquidator_accounts_starting_pos..liquidatee_accounts_starting_pos];

    // Verify liquidatee liquidation post health
    let post_liquidation_health =
        RiskEngine::new(&liquidatee_surroundfi_account, liquidatee_remaining_accounts)?
            .check_post_liquidation_condition_and_get_account_health(
                &ctx.accounts.liab_bank.key(),
                pre_liquidation_health,
            )?;

    // TODO consider if health cache update here is worth blowing the extra CU

    // Verify liquidator account health
    RiskEngine::check_account_init_health(
        &liquidator_surroundfi_account,
        liquidator_remaining_accounts,
        &mut None,
    )?;

    emit!(LendingAccountLiquidateEvent {
        header: AccountEventHeader {
            signer: Some(ctx.accounts.authority.key()),
            surroundfi_account: liquidator_surroundfi_account_loader.key(),
            surroundfi_account_authority: liquidator_surroundfi_account.authority,
            surroundfi_group: ctx.accounts.group.key(),
        },
        liquidatee_surroundfi_account: liquidatee_surroundfi_account_loader.key(),
        liquidatee_surroundfi_account_authority: liquidatee_surroundfi_account.authority,
        asset_bank: ctx.accounts.asset_bank.key(),
        asset_mint: ctx.accounts.asset_bank.load_mut()?.mint,
        liability_bank: ctx.accounts.liab_bank.key(),
        liability_mint: ctx.accounts.liab_bank.load_mut()?.mint,
        liquidatee_pre_health: pre_liquidation_health.to_num::<f64>(),
        liquidatee_post_health: post_liquidation_health.to_num::<f64>(),
        pre_balances,
        post_balances,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct LendingAccountLiquidate<'info> {
    pub group: AccountLoader<'info, SurroundfiGroup>,

    #[account(
        mut,
        has_one = group
    )]
    pub asset_bank: AccountLoader<'info, Bank>,

    #[account(
        mut,
        has_one = group
    )]
    pub liab_bank: AccountLoader<'info, Bank>,

    #[account(
        mut,
        has_one = group,
        has_one = authority
    )]
    pub liquidator_surroundfi_account: AccountLoader<'info, SurroundfiAccount>,

    pub authority: Signer<'info>,

    #[account(
        mut,
        has_one = group
    )]
    pub liquidatee_surroundfi_account: AccountLoader<'info, SurroundfiAccount>,

    /// CHECK: Seed constraint
    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_AUTHORITY_SEED.as_bytes(),
            liab_bank.key().as_ref(),
        ],
        bump = liab_bank.load()?.liquidity_vault_authority_bump
    )]
    pub bank_liquidity_vault_authority: AccountInfo<'info>,

    /// CHECK: Seed constraint
    #[account(
        mut,
        seeds = [
            LIQUIDITY_VAULT_SEED.as_bytes(),
            liab_bank.key().as_ref(),
        ],
        bump = liab_bank.load()?.liquidity_vault_bump
    )]
    pub bank_liquidity_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Seed constraint
    #[account(
        mut,
        seeds = [
            INSURANCE_VAULT_SEED.as_bytes(),
            liab_bank.key().as_ref(),
        ],
        bump = liab_bank.load()?.insurance_vault_bump
    )]
    pub bank_insurance_vault: AccountInfo<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}
