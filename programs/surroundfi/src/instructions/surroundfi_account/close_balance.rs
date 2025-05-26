use anchor_lang::prelude::*;

use crate::{
    check,
    prelude::*,
    state::{
        surroundfi_account::{BankAccountWrapper, SurroundfiAccount, ACCOUNT_DISABLED},
        surroundfi_group::Bank,
    },
};

pub fn lending_account_close_balance(ctx: Context<LendingAccountCloseBalance>) -> SurroundfiResult {
    let LendingAccountCloseBalance {
        surroundfi_account,
        bank: bank_loader,
        group: surroundfi_group_loader,
        ..
    } = ctx.accounts;

    let mut surroundfi_account = surroundfi_account.load_mut()?;
    let mut bank = bank_loader.load_mut()?;

    check!(
        !surroundfi_account.get_flag(ACCOUNT_DISABLED),
        SurroundfiError::AccountDisabled
    );

    bank.accrue_interest(
        Clock::get()?.unix_timestamp,
        &*surroundfi_group_loader.load()?,
        #[cfg(not(feature = "client"))]
        bank_loader.key(),
    )?;

    let mut bank_account = BankAccountWrapper::find(
        &bank_loader.key(),
        &mut bank,
        &mut surroundfi_account.lending_account,
    )?;

    bank_account.close_balance()?;

    Ok(())
}

#[derive(Accounts)]
pub struct LendingAccountCloseBalance<'info> {
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
        has_one = group
    )]
    pub bank: AccountLoader<'info, Bank>,
}
