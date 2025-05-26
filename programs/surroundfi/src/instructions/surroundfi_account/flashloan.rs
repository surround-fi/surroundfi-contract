use crate::{
    check,
    prelude::*,
    state::surroundfi_account::{
        SurroundfiAccount, RiskEngine, ACCOUNT_DISABLED, ACCOUNT_IN_FLASHLOAN,
    },
};
use anchor_lang::{prelude::*, Discriminator};
use solana_program::{
    instruction::{get_stack_height, TRANSACTION_LEVEL_STACK_HEIGHT},
    sysvar::{self, instructions},
};

pub fn lending_account_start_flashloan(
    ctx: Context<LendingAccountStartFlashloan>,
    end_index: u64,
) -> SurroundfiResult<()> {
    check_flashloan_can_start(
        &ctx.accounts.surroundfi_account,
        &ctx.accounts.ixs_sysvar,
        end_index as usize,
    )?;

    let mut surroundfi_account = ctx.accounts.surroundfi_account.load_mut()?;
    surroundfi_account.set_flag(ACCOUNT_IN_FLASHLOAN);

    Ok(())
}

#[derive(Accounts)]
pub struct LendingAccountStartFlashloan<'info> {
    #[account(
        mut,
        has_one = authority
    )]
    pub surroundfi_account: AccountLoader<'info, SurroundfiAccount>,

    pub authority: Signer<'info>,

    /// CHECK: Instructions sysvar
    #[account(address = sysvar::instructions::ID)]
    pub ixs_sysvar: AccountInfo<'info>,
}

const END_FL_IX_SURROUNDFI_ACCOUNT_AI_IDX: usize = 0;

/// Checklist
/// 1. `end_flashloan` ix index is after `start_flashloan` ix index
/// 2. Ixs has an `end_flashloan` ix present
/// 3. `end_flashloan` ix is for the surroundfi program
/// 3. `end_flashloan` ix is for the same surroundfi account
/// 4. Account is not disabled
/// 5. Account is not already in a flashloan
/// 6. Start flashloan ix is not in CPI
/// 7. End flashloan ix is not in CPI
pub fn check_flashloan_can_start(
    surroundfi_account: &AccountLoader<SurroundfiAccount>,
    sysvar_ixs: &AccountInfo,
    end_fl_idx: usize,
) -> SurroundfiResult<()> {
    // Note: FLASHLOAN_ENABLED_FLAG is now deprecated, any non-disabled account can initiate a flash loan.
    let current_ix_idx: usize = instructions::load_current_index_checked(sysvar_ixs)?.into();

    check!(current_ix_idx < end_fl_idx, SurroundfiError::IllegalFlashloan);

    // Check current ix is not a CPI
    let current_ix = instructions::load_instruction_at_checked(current_ix_idx, sysvar_ixs)?;

    check!(
        get_stack_height() == TRANSACTION_LEVEL_STACK_HEIGHT,
        SurroundfiError::IllegalFlashloan,
        "Start flashloan ix should not be in CPI"
    );

    check!(
        current_ix.program_id.eq(&crate::id()),
        SurroundfiError::IllegalFlashloan,
        "Start flashloan ix should not be in CPI"
    );

    // Will error if ix doesn't exist
    let unchecked_end_fl_ix = instructions::load_instruction_at_checked(end_fl_idx, sysvar_ixs)?;

    check!(
        unchecked_end_fl_ix.data[..8]
            .eq(&crate::instruction::LendingAccountEndFlashloan::DISCRIMINATOR),
        SurroundfiError::IllegalFlashloan
    );

    check!(
        unchecked_end_fl_ix.program_id.eq(&crate::id()),
        SurroundfiError::IllegalFlashloan
    );

    let end_fl_ix = unchecked_end_fl_ix;

    let end_fl_surroundfi_account = end_fl_ix
        .accounts
        .get(END_FL_IX_SURROUNDFI_ACCOUNT_AI_IDX)
        .ok_or(SurroundfiError::IllegalFlashloan)?;

    check!(
        end_fl_surroundfi_account.pubkey.eq(&surroundfi_account.key()),
        SurroundfiError::IllegalFlashloan
    );

    let surroundfi_account = surroundfi_account.load()?;

    check!(
        !surroundfi_account.get_flag(ACCOUNT_DISABLED),
        SurroundfiError::AccountDisabled
    );

    check!(
        !surroundfi_account.get_flag(ACCOUNT_IN_FLASHLOAN),
        SurroundfiError::IllegalFlashloan
    );

    Ok(())
}

pub fn lending_account_end_flashloan<'info>(
    ctx: Context<'_, '_, 'info, 'info, LendingAccountEndFlashloan<'info>>,
) -> SurroundfiResult<()> {
    check!(
        get_stack_height() == TRANSACTION_LEVEL_STACK_HEIGHT,
        SurroundfiError::IllegalFlashloan,
        "End flashloan ix should not be in CPI"
    );

    let mut surroundfi_account = ctx.accounts.surroundfi_account.load_mut()?;

    surroundfi_account.unset_flag(ACCOUNT_IN_FLASHLOAN);

    RiskEngine::check_account_init_health(&surroundfi_account, ctx.remaining_accounts, &mut None)?;

    Ok(())
}

#[derive(Accounts)]
pub struct LendingAccountEndFlashloan<'info> {
    #[account(
        mut,
        has_one = authority
    )]
    pub surroundfi_account: AccountLoader<'info, SurroundfiAccount>,

    pub authority: Signer<'info>,
}
