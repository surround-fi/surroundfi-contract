// Global fee admin calls this to edit the fee rate or the fee wallet.

use crate::constants::FEE_STATE_SEED;
use crate::state::fee_state;
use crate::state::surroundfi_group::WrappedI80F48;
use anchor_lang::prelude::*;
use fee_state::FeeState;

pub fn edit_fee_state(
    ctx: Context<EditFeeState>,
    admin: Pubkey,
    fee_wallet: Pubkey,
    bank_init_flat_sol_fee: u32,
    program_fee_fixed: WrappedI80F48,
    program_fee_rate: WrappedI80F48,
) -> Result<()> {
    let mut fee_state = ctx.accounts.fee_state.load_mut()?;
    fee_state.global_fee_admin = admin;
    fee_state.global_fee_wallet = fee_wallet;
    fee_state.bank_init_flat_sol_fee = bank_init_flat_sol_fee;
    fee_state.program_fee_fixed = program_fee_fixed;
    fee_state.program_fee_rate = program_fee_rate;

    let fixed = u128::from_le_bytes(fee_state.program_fee_fixed.value);
    let rate = u128::from_le_bytes(fee_state.program_fee_rate.value);
    msg!("admin set to: {:?} fee wallet: {:?}", admin, fee_wallet);
    msg!(
        "flat sol: {:?} fixed: {:?} rate: {:?}",
        fee_state.bank_init_flat_sol_fee,
        fixed,
        rate
    );

    Ok(())
}

#[derive(Accounts)]
pub struct EditFeeState<'info> {
    /// Admin of the global FeeState
    #[account(mut)]
    pub global_fee_admin: Signer<'info>,

    // Note: there is just one FeeState per program, so no further check is required.
    #[account(
        mut,
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump,
        has_one = global_fee_admin
    )]
    pub fee_state: AccountLoader<'info, FeeState>,
}
