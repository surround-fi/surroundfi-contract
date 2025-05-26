use anchor_lang::prelude::*;
use bytemuck::Zeroable;
use solana_program::{clock::Clock, sysvar::Sysvar};

use crate::{
    state::{
        health_cache::HealthCache,
        surroundfi_account::{SurroundfiAccount, RiskEngine},
    },
    SurroundfiResult,
};

pub fn lending_account_pulse_health<'info>(
    ctx: Context<'_, '_, 'info, 'info, PulseHealth<'info>>,
) -> SurroundfiResult {
    let clock = Clock::get()?;
    let mut surroundfi_account = ctx.accounts.surroundfi_account.load_mut()?;

    let mut health_cache = HealthCache::zeroed();
    health_cache.timestamp = clock.unix_timestamp;

    match RiskEngine::check_account_init_health(
        &surroundfi_account,
        ctx.remaining_accounts,
        &mut Some(&mut health_cache),
    ) {
        Ok(()) => {
            health_cache.set_engine_ok(true);
        }
        Err(_) => {
            health_cache.set_engine_ok(false);
        }
    }

    surroundfi_account.health_cache = health_cache;

    Ok(())
}

#[derive(Accounts)]
pub struct PulseHealth<'info> {
    #[account(mut)]
    pub surroundfi_account: AccountLoader<'info, SurroundfiAccount>,
}
