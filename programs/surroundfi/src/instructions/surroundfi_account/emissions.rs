use anchor_lang::{prelude::*, Accounts, ToAccountInfo};
use anchor_spl::{
    associated_token::get_associated_token_address_with_program_id,
    token_interface::{transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked},
};

use crate::{
    check,
    constants::{EMISSIONS_AUTH_SEED, EMISSIONS_TOKEN_ACCOUNT_SEED},
    debug,
    prelude::{SurroundfiError, SurroundfiResult},
    state::{
        surroundfi_account::{BankAccountWrapper, SurroundfiAccount, ACCOUNT_DISABLED},
        surroundfi_group::{Bank, SurroundfiGroup},
    },
};

pub fn lending_account_withdraw_emissions<'info>(
    ctx: Context<'_, '_, 'info, 'info, LendingAccountWithdrawEmissions<'info>>,
) -> SurroundfiResult {
    let mut surroundfi_account = ctx.accounts.surroundfi_account.load_mut()?;

    check!(
        !surroundfi_account.get_flag(ACCOUNT_DISABLED),
        SurroundfiError::AccountDisabled
    );

    let mut bank = ctx.accounts.bank.load_mut()?;

    let mut balance = BankAccountWrapper::find(
        ctx.accounts.bank.to_account_info().key,
        &mut bank,
        &mut surroundfi_account.lending_account,
    )?;

    // Settle emissions
    let emissions_settle_amount = balance.settle_emissions_and_get_transfer_amount()?;

    if emissions_settle_amount > 0 {
        debug!("Transferring {} emissions to user", emissions_settle_amount);

        let signer_seeds: &[&[&[u8]]] = &[&[
            EMISSIONS_AUTH_SEED.as_bytes(),
            &ctx.accounts.bank.key().to_bytes(),
            &ctx.accounts.emissions_mint.key().to_bytes(),
            &[ctx.bumps.emissions_auth],
        ]];

        transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.emissions_vault.to_account_info(),
                    to: ctx.accounts.destination_account.to_account_info(),
                    authority: ctx.accounts.emissions_auth.to_account_info(),
                    mint: ctx.accounts.emissions_mint.to_account_info(),
                },
                signer_seeds,
            ),
            emissions_settle_amount,
            ctx.accounts.emissions_mint.decimals,
        )?;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct LendingAccountWithdrawEmissions<'info> {
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
        has_one = emissions_mint
    )]
    pub bank: AccountLoader<'info, Bank>,

    pub emissions_mint: InterfaceAccount<'info, Mint>,

    /// CHECK: PDA seeds validated
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
        mut,
        seeds = [
            EMISSIONS_TOKEN_ACCOUNT_SEED.as_bytes(),
            bank.key().as_ref(),
            emissions_mint.key().as_ref(),
        ],
        bump,
    )]
    pub emissions_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub destination_account: Box<InterfaceAccount<'info, TokenAccount>>,
    pub token_program: Interface<'info, TokenInterface>,
}

/// Permissionlessly settle unclaimed emissions to a users account.
pub fn lending_account_settle_emissions(
    ctx: Context<LendingAccountSettleEmissions>,
) -> SurroundfiResult {
    let mut surroundfi_account = ctx.accounts.surroundfi_account.load_mut()?;
    let mut bank = ctx.accounts.bank.load_mut()?;

    let mut balance = BankAccountWrapper::find(
        ctx.accounts.bank.to_account_info().key,
        &mut bank,
        &mut surroundfi_account.lending_account,
    )?;

    balance.claim_emissions(Clock::get()?.unix_timestamp.try_into().unwrap())?;

    Ok(())
}

#[derive(Accounts)]
pub struct LendingAccountSettleEmissions<'info> {
    #[account(
        mut,
        constraint = surroundfi_account.load()?.group == bank.load()?.group,
    )]
    pub surroundfi_account: AccountLoader<'info, SurroundfiAccount>,

    #[account(mut)]
    pub bank: AccountLoader<'info, Bank>,
}

/// emissions rewards will be withdrawn to the emissions_destination_account
pub fn surroundfi_account_update_emissions_destination_account<'info>(
    ctx: Context<'_, '_, 'info, 'info, SurroundfiAccountUpdateEmissionsDestinationAccount<'info>>,
) -> SurroundfiResult {
    let mut surroundfi_account = ctx.accounts.surroundfi_account.load_mut()?;

    check!(
        !surroundfi_account.get_flag(ACCOUNT_DISABLED),
        SurroundfiError::AccountDisabled
    );

    surroundfi_account.emissions_destination_account = ctx.accounts.destination_account.key();

    Ok(())
}

#[derive(Accounts)]
pub struct SurroundfiAccountUpdateEmissionsDestinationAccount<'info> {
    #[account(
        mut,
        has_one = authority
    )]
    pub surroundfi_account: AccountLoader<'info, SurroundfiAccount>,

    pub authority: Signer<'info>,

    /// User's earned emissions will be sent to the cannonical ATA of this wallet.
    ///
    /// CHECK: Completely unchecked, user picks a destination without restrictions
    pub destination_account: AccountInfo<'info>,
}

/// Permissionlessly withdraw emissions to user emissions_destination_account
pub fn lending_account_withdraw_emissions_permissionless<'info>(
    ctx: Context<'_, '_, 'info, 'info, LendingAccountWithdrawEmissionsPermissionless<'info>>,
) -> SurroundfiResult {
    let mut surroundfi_account = ctx.accounts.surroundfi_account.load_mut()?;

    check!(
        !surroundfi_account.get_flag(ACCOUNT_DISABLED),
        SurroundfiError::AccountDisabled
    );

    let emissions_dest_wallet = &surroundfi_account.emissions_destination_account;
    let emissions_mint = &ctx.accounts.emissions_mint.key();
    let emissions_token_program = &ctx.accounts.token_program.key();

    // Ensure that the emissions_destination_account was previously set by the user
    check!(
        !emissions_dest_wallet.eq(&Pubkey::default()),
        SurroundfiError::InvalidEmissionsDestinationAccount
    );

    // Ensure the destination is the cannonical ATA of the user-specified wallet
    let ata_expected = get_associated_token_address_with_program_id(
        emissions_dest_wallet,
        emissions_mint,
        emissions_token_program,
    );
    check!(
        ata_expected == ctx.accounts.destination_account.key(),
        SurroundfiError::InvalidEmissionsDestinationAccount
    );

    let mut bank = ctx.accounts.bank.load_mut()?;

    let mut bank_account = BankAccountWrapper::find(
        ctx.accounts.bank.to_account_info().key,
        &mut bank,
        &mut surroundfi_account.lending_account,
    )?;

    // Settle emissions
    let emissions_settle_amount = bank_account.settle_emissions_and_get_transfer_amount()?;

    if emissions_settle_amount > 0 {
        debug!("Transferring {} emissions to user", emissions_settle_amount);

        let signer_seeds: &[&[&[u8]]] = &[&[
            EMISSIONS_AUTH_SEED.as_bytes(),
            &ctx.accounts.bank.key().to_bytes(),
            &ctx.accounts.emissions_mint.key().to_bytes(),
            &[ctx.bumps.emissions_auth],
        ]];

        transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.emissions_vault.to_account_info(),
                    to: ctx.accounts.destination_account.to_account_info(),
                    authority: ctx.accounts.emissions_auth.to_account_info(),
                    mint: ctx.accounts.emissions_mint.to_account_info(),
                },
                signer_seeds,
            ),
            emissions_settle_amount,
            ctx.accounts.emissions_mint.decimals,
        )?;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct LendingAccountWithdrawEmissionsPermissionless<'info> {
    pub group: AccountLoader<'info, SurroundfiGroup>,

    #[account(
        mut,
        has_one = group
    )]
    pub surroundfi_account: AccountLoader<'info, SurroundfiAccount>,

    #[account(
        mut,
        has_one = group,
        has_one = emissions_mint
    )]
    pub bank: AccountLoader<'info, Bank>,

    pub emissions_mint: InterfaceAccount<'info, Mint>,

    #[account(
        seeds = [
            EMISSIONS_AUTH_SEED.as_bytes(),
            bank.key().as_ref(),
            emissions_mint.key().as_ref(),
        ],
        bump
    )]
    /// CHECK: Asserted by PDA
    pub emissions_auth: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [
            EMISSIONS_TOKEN_ACCOUNT_SEED.as_bytes(),
            bank.key().as_ref(),
            emissions_mint.key().as_ref(),
        ],
        bump,
    )]
    pub emissions_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK: Handler will validate this is cannonical ATA of the `emissions_destination_account`
    /// registered on `surroundfi_account`
    #[account(mut)]
    pub destination_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
}
