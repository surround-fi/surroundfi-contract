use crate::{constants::FEE_STATE_SEED, state::fee_state::FeeState, SurroundfiGroup, SurroundfiResult};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct ConfigGroupFee<'info> {
    #[account(mut)]
    pub surroundfi_group: AccountLoader<'info, SurroundfiGroup>,

    /// `global_fee_admin` of the FeeState
    pub global_fee_admin: Signer<'info>,

    // Note: there is just one FeeState per program, so no further check is required.
    #[account(
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump,
        has_one = global_fee_admin
    )]
    pub fee_state: AccountLoader<'info, FeeState>,
}

pub fn config_group_fee(ctx: Context<ConfigGroupFee>, enable_program_fee: bool) -> SurroundfiResult {
    let mut surroundfi_group = ctx.accounts.surroundfi_group.load_mut()?;
    let flag_before = surroundfi_group.group_flags;

    surroundfi_group.set_program_fee_enabled(enable_program_fee);

    msg!(
        "flag set to: {:?} was {:?}",
        surroundfi_group.group_flags,
        flag_before
    );

    Ok(())
}
