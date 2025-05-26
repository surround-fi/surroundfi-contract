use crate::constants::FEE_STATE_SEED;
use crate::events::{GroupEventHeader, SurroundfiGroupCreateEvent};
use crate::state::fee_state::FeeState;
use crate::{state::surroundfi_group::SurroundfiGroup, SurroundfiResult};
use anchor_lang::prelude::*;

pub fn initialize_group(
    ctx: Context<SurroundfiGroupInitialize>,
    is_arena_group: bool,
) -> SurroundfiResult {
    let surroundfi_group = &mut ctx.accounts.surroundfi_group.load_init()?;

    surroundfi_group.set_initial_configuration(ctx.accounts.admin.key());
    surroundfi_group.set_arena_group(is_arena_group)?;

    msg!(
        "Group admin: {:?} flags: {:?}",
        surroundfi_group.admin,
        surroundfi_group.group_flags
    );

    let fee_state = ctx.accounts.fee_state.load()?;

    surroundfi_group.fee_state_cache.global_fee_wallet = fee_state.global_fee_wallet;
    surroundfi_group.fee_state_cache.program_fee_fixed = fee_state.program_fee_fixed;
    surroundfi_group.fee_state_cache.program_fee_rate = fee_state.program_fee_rate;
    surroundfi_group.banks = 0;

    let cache = surroundfi_group.fee_state_cache;
    msg!(
        "global fee wallet: {:?}, fixed fee: {:?}, program free {:?}",
        cache.global_fee_wallet,
        cache.program_fee_fixed,
        cache.program_fee_rate
    );

    emit!(SurroundfiGroupCreateEvent {
        header: GroupEventHeader {
            surroundfi_group: ctx.accounts.surroundfi_group.key(),
            signer: Some(*ctx.accounts.admin.key)
        },
    });

    Ok(())
}

#[derive(Accounts)]
pub struct SurroundfiGroupInitialize<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + std::mem::size_of::<SurroundfiGroup>(),
    )]
    pub surroundfi_group: AccountLoader<'info, SurroundfiGroup>,

    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        seeds = [FEE_STATE_SEED.as_bytes()],
        bump,
    )]
    pub fee_state: AccountLoader<'info, FeeState>,

    pub system_program: Program<'info, System>,
}
