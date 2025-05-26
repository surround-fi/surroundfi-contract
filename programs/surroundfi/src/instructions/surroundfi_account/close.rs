use anchor_lang::prelude::*;

use crate::{check, state::surroundfi_account::SurroundfiAccount, SurroundfiError, SurroundfiResult};

pub fn close_account(ctx: Context<SurroundfiAccountClose>) -> SurroundfiResult {
    let surroundfi_account = &ctx.accounts.surroundfi_account.load()?;

    check!(
        surroundfi_account.can_be_closed(),
        SurroundfiError::IllegalAction,
        "Account cannot be closed"
    );

    Ok(())
}

#[derive(Accounts)]
pub struct SurroundfiAccountClose<'info> {
    #[account(
        mut,
        has_one = authority,
        close = fee_payer
    )]
    pub surroundfi_account: AccountLoader<'info, SurroundfiAccount>,

    pub authority: Signer<'info>,
    #[account(mut)]
    pub fee_payer: Signer<'info>,
}
