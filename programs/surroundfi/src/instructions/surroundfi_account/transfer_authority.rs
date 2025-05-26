use crate::{prelude::*, state::surroundfi_account::SurroundfiAccount};
use anchor_lang::prelude::*;

pub fn set_account_transfer_authority(
    ctx: Context<SurroundfiAccountSetAccountAuthority>,
) -> SurroundfiResult {
    // Ensure surroundfi_account is dropped out of scope to not exceed stack frame limits
    {
        let mut surroundfi_account = ctx.accounts.surroundfi_account.load_mut()?;
        let new_account_authority = ctx.accounts.new_authority.key();
        surroundfi_account.set_new_account_authority_checked(new_account_authority)?;
    }

    // TODO: add back event (dropped for memory reasons)

    Ok(())
}

#[derive(Accounts)]
pub struct SurroundfiAccountSetAccountAuthority<'info> {
    #[account(
        mut,
        has_one = authority,
        has_one = group
    )]
    pub surroundfi_account: AccountLoader<'info, SurroundfiAccount>,

    /// CHECK: Validated against account
    pub group: AccountInfo<'info>,

    pub authority: Signer<'info>,

    /// CHECK: The new account authority doesn't need explicit checks
    pub new_authority: AccountInfo<'info>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,
}
