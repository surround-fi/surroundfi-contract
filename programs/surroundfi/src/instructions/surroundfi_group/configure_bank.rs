use crate::constants::{EMISSIONS_AUTH_SEED, EMISSIONS_TOKEN_ACCOUNT_SEED, FREEZE_SETTINGS};
use crate::events::{
    GroupEventHeader, LendingPoolBankConfigureEvent, LendingPoolBankConfigureFrozenEvent,
};
use crate::prelude::SurroundfiError;
use crate::{check, math_error, utils};
use crate::{
    state::surroundfi_group::{Bank, BankConfigOpt, SurroundfiGroup},
    SurroundfiResult,
};
use anchor_lang::prelude::*;
use anchor_spl::token_2022::{transfer_checked, TransferChecked};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};
use fixed::types::I80F48;

pub fn lending_pool_configure_bank(
    ctx: Context<LendingPoolConfigureBank>,
    bank_config: BankConfigOpt,
) -> SurroundfiResult {
    let mut bank = ctx.accounts.bank.load_mut()?;

    // If settings are frozen, you can only update the deposit and borrow limits, everything else is ignored.
    if bank.get_flag(FREEZE_SETTINGS) {
        bank.configure_unfrozen_fields_only(&bank_config)?;

        msg!("WARN: Only deposit+borrow limits updated. Other settings IGNORED for frozen banks!");

        emit!(LendingPoolBankConfigureFrozenEvent {
            header: GroupEventHeader {
                surroundfi_group: ctx.accounts.group.key(),
                signer: Some(*ctx.accounts.admin.key)
            },
            bank: ctx.accounts.bank.key(),
            mint: bank.mint,
            deposit_limit: bank.config.deposit_limit,
            borrow_limit: bank.config.borrow_limit,
        });
    } else {
        // Settings are not frozen, everything updates
        bank.configure(&bank_config)?;

        if bank_config.oracle_max_age.is_some() {
            bank.config.validate_oracle_age()?;
        }

        emit!(LendingPoolBankConfigureEvent {
            header: GroupEventHeader {
                surroundfi_group: ctx.accounts.group.key(),
                signer: Some(*ctx.accounts.admin.key)
            },
            bank: ctx.accounts.bank.key(),
            mint: bank.mint,
            config: bank_config,
        });
    }

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolConfigureBank<'info> {
    #[account(
        mut,
        has_one = admin,
    )]
    pub group: AccountLoader<'info, SurroundfiGroup>,

    pub admin: Signer<'info>,

    #[account(
        mut,
        has_one = group,
    )]
    pub bank: AccountLoader<'info, Bank>,
}

pub fn lending_pool_setup_emissions(
    ctx: Context<LendingPoolSetupEmissions>,
    emissions_flags: u64,
    emissions_rate: u64,
    total_emissions: u64,
) -> SurroundfiResult {
    let mut bank = ctx.accounts.bank.load_mut()?;

    check!(
        bank.emissions_mint.eq(&Pubkey::default()),
        SurroundfiError::EmissionsAlreadySetup
    );

    bank.emissions_mint = ctx.accounts.emissions_mint.key();

    bank.override_emissions_flag(emissions_flags);

    bank.emissions_rate = emissions_rate;
    bank.emissions_remaining = I80F48::from_num(total_emissions).into();

    msg!("init emissions with mint: {:?}", bank.emissions_mint,);
    msg!(
        "flags: {:?} rate: {:?} total: {:?}",
        emissions_flags,
        emissions_rate,
        total_emissions
    );

    let initial_emissions_amount_pre_fee = utils::calculate_pre_fee_spl_deposit_amount(
        ctx.accounts.emissions_mint.to_account_info(),
        total_emissions,
        Clock::get()?.epoch,
    )?;

    transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.emissions_funding_account.to_account_info(),
                to: ctx.accounts.emissions_token_account.to_account_info(),
                authority: ctx.accounts.admin.to_account_info(),
                mint: ctx.accounts.emissions_mint.to_account_info(),
            },
        ),
        initial_emissions_amount_pre_fee,
        ctx.accounts.emissions_mint.decimals,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolSetupEmissions<'info> {
    #[account(
        mut,
        has_one = admin,
    )]
    pub group: AccountLoader<'info, SurroundfiGroup>,

    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        has_one = group,
    )]
    pub bank: AccountLoader<'info, Bank>,

    pub emissions_mint: InterfaceAccount<'info, Mint>,

    /// CHECK: Asserted by PDA constraints
    #[account(
        seeds = [
            EMISSIONS_AUTH_SEED.as_bytes(),
            bank.key().as_ref(),
            emissions_mint.key().as_ref(),
        ],
        bump
    )]
    pub emissions_auth: AccountInfo<'info>,

    #[account(
        init,
        payer = admin,
        token::mint = emissions_mint,
        token::authority = emissions_auth,
        seeds = [
            EMISSIONS_TOKEN_ACCOUNT_SEED.as_bytes(),
            bank.key().as_ref(),
            emissions_mint.key().as_ref(),
        ],
        bump,
    )]
    pub emissions_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// NOTE: This is a TokenAccount, spl transfer will validate it.
    ///
    /// CHECK: Account provided only for funding rewards
    #[account(mut)]
    pub emissions_funding_account: AccountInfo<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn lending_pool_update_emissions_parameters(
    ctx: Context<LendingPoolUpdateEmissionsParameters>,
    emissions_flags: Option<u64>,
    emissions_rate: Option<u64>,
    additional_emissions: Option<u64>,
) -> SurroundfiResult {
    let mut bank = ctx.accounts.bank.load_mut()?;

    check!(
        bank.emissions_mint.ne(&Pubkey::default()),
        SurroundfiError::EmissionsUpdateError
    );

    check!(
        bank.emissions_mint.eq(&ctx.accounts.emissions_mint.key()),
        SurroundfiError::EmissionsUpdateError
    );

    if let Some(flags) = emissions_flags {
        msg!("Updating emissions flags to {:#010b}", flags);
        bank.flags = flags;
    }

    if let Some(rate) = emissions_rate {
        msg!("Updating emissions rate to {}", rate);
        bank.emissions_rate = rate;
    }

    if let Some(additional_emissions) = additional_emissions {
        bank.emissions_remaining = I80F48::from(bank.emissions_remaining)
            .checked_add(I80F48::from_num(additional_emissions))
            .ok_or_else(math_error!())?
            .into();

        msg!(
            "Adding {} emissions, total {}",
            additional_emissions,
            I80F48::from(bank.emissions_remaining)
        );

        let additional_emissions_amount_pre_fee = utils::calculate_pre_fee_spl_deposit_amount(
            ctx.accounts.emissions_mint.to_account_info(),
            additional_emissions,
            Clock::get()?.epoch,
        )?;

        transfer_checked(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.emissions_funding_account.to_account_info(),
                    to: ctx.accounts.emissions_token_account.to_account_info(),
                    authority: ctx.accounts.admin.to_account_info(),
                    mint: ctx.accounts.emissions_mint.to_account_info(),
                },
            ),
            additional_emissions_amount_pre_fee,
            ctx.accounts.emissions_mint.decimals,
        )?;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolUpdateEmissionsParameters<'info> {
    #[account(
        mut,
        has_one = admin
    )]
    pub group: AccountLoader<'info, SurroundfiGroup>,

    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        has_one = group
    )]
    pub bank: AccountLoader<'info, Bank>,

    pub emissions_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        seeds = [
            EMISSIONS_TOKEN_ACCOUNT_SEED.as_bytes(),
            bank.key().as_ref(),
            emissions_mint.key().as_ref(),
        ],
        bump,
    )]
    pub emissions_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Account provided only for funding rewards
    #[account(mut)]
    pub emissions_funding_account: AccountInfo<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}
