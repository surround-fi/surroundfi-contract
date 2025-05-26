use crate::{
    state::surroundfi_group::{Bank, SurroundfiGroup},
    SurroundfiResult,
};
use anchor_lang::prelude::*;

pub fn lending_pool_accrue_bank_interest(
    ctx: Context<LendingPoolAccrueBankInterest>,
) -> SurroundfiResult {
    let clock = Clock::get()?;
    let mut bank = ctx.accounts.bank.load_mut()?;

    bank.accrue_interest(
        clock.unix_timestamp,
        &*ctx.accounts.group.load()?,
        #[cfg(not(feature = "client"))]
        ctx.accounts.bank.key(),
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct LendingPoolAccrueBankInterest<'info> {
    pub group: AccountLoader<'info, SurroundfiGroup>,

    #[account(
        mut,
        has_one = group
    )]
    pub bank: AccountLoader<'info, Bank>,
}
