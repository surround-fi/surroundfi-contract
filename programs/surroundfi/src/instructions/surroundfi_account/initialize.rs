use crate::{
    events::{AccountEventHeader, SurroundfiAccountCreateEvent},
    prelude::*,
    state::surroundfi_account::SurroundfiAccount,
};
use anchor_lang::prelude::*;
use solana_program::sysvar::Sysvar;

pub fn initialize_account(ctx: Context<SurroundfiAccountInitialize>) -> SurroundfiResult {
    let SurroundfiAccountInitialize {
        authority,
        surroundfi_group,
        surroundfi_account: surroundfi_account_loader,
        ..
    } = ctx.accounts;

    let mut surroundfi_account = surroundfi_account_loader.load_init()?;

    surroundfi_account.initialize(surroundfi_group.key(), authority.key());

    emit!(SurroundfiAccountCreateEvent {
        header: AccountEventHeader {
            signer: Some(authority.key()),
            surroundfi_account: surroundfi_account_loader.key(),
            surroundfi_account_authority: surroundfi_account.authority,
            surroundfi_group: surroundfi_account.group,
        }
    });

    Ok(())
}

#[derive(Accounts)]
pub struct SurroundfiAccountInitialize<'info> {
    pub surroundfi_group: AccountLoader<'info, SurroundfiGroup>,

    #[account(
        init,
        payer = fee_payer,
        space = 8 + std::mem::size_of::<SurroundfiAccount>()
    )]
    pub surroundfi_account: AccountLoader<'info, SurroundfiAccount>,

    pub authority: Signer<'info>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}
