use crate::{
    constants::{DEPOSIT_MFI_AUTH_SIGNER_SEED, SURROUNDFI_ACCOUNT_SEED},
    errors::LIPError,
    state::{Campaign, Deposit},
};
use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::{close_account, CloseAccount},
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use surroundfi::{program::Surroundfi, state::surroundfi_group::Bank};
use std::mem::size_of;

/// Creates a new deposit in an active liquidity incentive campaign (LIP).
///
/// # Arguments
/// * `ctx`: Context struct containing the relevant accounts for the new deposit
/// * `amount`: The amount of tokens to be deposited.
///
/// # Returns
/// * `Ok(())` if the deposit was successfully made, or an error otherwise.
///
/// # Errors
/// * `LIPError::CampaignNotActive` if the relevant campaign is not active.
/// * `LIPError::DepositAmountTooLarge` is the deposit amount exceeds the amount of remaining deposits that can be made into the campaign.
pub fn process<'info>(
    ctx: Context<'_, '_, '_, 'info, CreateDeposit<'info>>,
    amount: u64,
) -> Result<()> {
    require!(ctx.accounts.campaign.active, LIPError::CampaignNotActive);

    require_gte!(
        ctx.accounts.campaign.remaining_capacity,
        amount,
        LIPError::DepositAmountTooLarge
    );

    require_gt!(amount, 0);

    msg!("User depositing {} tokens", amount);

    anchor_spl::token_2022::spl_token_2022::onchain::invoke_transfer_checked(
        ctx.accounts.token_program.key,
        ctx.accounts.funding_account.to_account_info(),
        ctx.accounts.asset_mint.to_account_info(),
        ctx.accounts.temp_token_account.to_account_info(),
        ctx.accounts.signer.to_account_info(),
        ctx.remaining_accounts,
        amount,
        ctx.accounts.asset_mint.decimals,
        &[], // seeds
    )?;

    let mfi_signer_seeds: &[&[u8]] = &[
        DEPOSIT_MFI_AUTH_SIGNER_SEED.as_bytes(),
        &ctx.accounts.deposit.key().to_bytes(),
        &[ctx.bumps.mfi_pda_signer],
    ];

    surroundfi::cpi::surroundfi_account_initialize(CpiContext::new_with_signer(
        ctx.accounts.surroundfi_program.to_account_info(),
        surroundfi::cpi::accounts::SurroundfiAccountInitialize {
            surroundfi_group: ctx.accounts.surroundfi_group.to_account_info(),
            authority: ctx.accounts.mfi_pda_signer.to_account_info(),
            surroundfi_account: ctx.accounts.surroundfi_account.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            fee_payer: ctx.accounts.signer.to_account_info(),
        },
        &[
            mfi_signer_seeds,
            &[
                SURROUNDFI_ACCOUNT_SEED.as_bytes(),
                &ctx.accounts.deposit.key().to_bytes(),
                &[ctx.bumps.surroundfi_account],
            ],
        ],
    ))?;

    let signer_seeds = &[mfi_signer_seeds];
    let mut cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.surroundfi_program.to_account_info(),
        surroundfi::cpi::accounts::LendingAccountDeposit {
            group: ctx.accounts.surroundfi_group.to_account_info(),
            surroundfi_account: ctx.accounts.surroundfi_account.to_account_info(),
            authority: ctx.accounts.mfi_pda_signer.to_account_info(),
            bank: ctx.accounts.surroundfi_bank.to_account_info(),
            signer_token_account: ctx.accounts.temp_token_account.to_account_info(),
            liquidity_vault: ctx.accounts.surroundfi_bank_vault.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        },
        signer_seeds,
    );
    cpi_ctx.remaining_accounts = ctx.remaining_accounts.to_vec();

    if surroundfi::utils::nonzero_fee(
        ctx.accounts.asset_mint.to_account_info(),
        Clock::get()?.epoch,
    )? {
        msg!("nonzero transfer fee not supported");
        return Err(ProgramError::InvalidAccountData.into());
    }

    surroundfi::cpi::lending_account_deposit(cpi_ctx, amount, None)?;

    close_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        CloseAccount {
            account: ctx.accounts.temp_token_account.to_account_info(),
            destination: ctx.accounts.signer.to_account_info(),
            authority: ctx.accounts.mfi_pda_signer.to_account_info(),
        },
        &[mfi_signer_seeds],
    ))?;

    ctx.accounts.deposit.set_inner(Deposit {
        owner: ctx.accounts.signer.key(),
        campaign: ctx.accounts.campaign.key(),
        amount,
        start_time: Clock::get()?.unix_timestamp,
        _padding: [0; 16],
    });

    ctx.accounts.campaign.remaining_capacity = ctx
        .accounts
        .campaign
        .remaining_capacity
        .checked_sub(amount)
        .unwrap();

    Ok(())
}

#[derive(Accounts)]
pub struct CreateDeposit<'info> {
    #[account(mut)]
    pub campaign: Box<Account<'info, Campaign>>,

    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(
        init,
        payer = signer,
        space = size_of::<Deposit>() + 8,
    )]
    pub deposit: Box<Account<'info, Deposit>>,

    #[account(
        seeds = [
            DEPOSIT_MFI_AUTH_SIGNER_SEED.as_bytes(),
            deposit.key().as_ref(),
        ],
        bump,
    )]
    /// CHECK: Asserted by PDA derivation
    pub mfi_pda_signer: AccountInfo<'info>,

    #[account(mut)]
    /// CHECK: Asserted by token transfer
    pub funding_account: AccountInfo<'info>,

    #[account(
        init,
        payer = signer,
        token::mint = asset_mint,
        token::authority = mfi_pda_signer,
    )]
    pub temp_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(address = surroundfi_bank.load()?.mint)]
    pub asset_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Asserted by mfi cpi call
    /// surroundfi_bank is tied to a specific surroundfi_group
    pub surroundfi_group: AccountInfo<'info>,

    #[account(
        mut,
        address = campaign.surroundfi_bank_pk,
    )]
    /// CHECK: Asserted by stored address
    pub surroundfi_bank: AccountLoader<'info, Bank>,

    /// CHECK: Asserted by CPI call
    #[account(
        mut,
        seeds = [
            SURROUNDFI_ACCOUNT_SEED.as_bytes(),
            deposit.key().as_ref(),
        ],
        bump,
    )]
    pub surroundfi_account: AccountInfo<'info>,

    #[account(mut)]
    /// CHECK: Asserted by CPI call,
    /// surroundfi_bank_vault is tied to a specific surroundfi_bank,
    /// passing in an incorrect vault will fail the CPI call
    pub surroundfi_bank_vault: AccountInfo<'info>,

    /// CHECK: Asserted by CPI call
    pub surroundfi_program: Program<'info, Surroundfi>,
    pub token_program: Interface<'info, TokenInterface>,
    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
}
