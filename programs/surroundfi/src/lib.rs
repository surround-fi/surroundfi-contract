pub mod constants;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod macros;
pub mod prelude;
pub mod state;
pub mod utils;

use anchor_lang::prelude::*;
use instructions::*;
use prelude::*;
use state::surroundfi_group::WrappedI80F48;
use state::surroundfi_group::{BankConfigCompact, BankConfigOpt};

declare_id!("DinAro7LsnoGwfdfq68N3Mf1RgtVMCHoJeQ2btM9Q137");

#[program]
pub mod surroundfi {
    use super::*;

    pub fn surroundfi_group_initialize(
        ctx: Context<SurroundfiGroupInitialize>,
        is_arena_group: bool,
    ) -> SurroundfiResult {
        surroundfi_group::initialize_group(ctx, is_arena_group)
    }

    pub fn surroundfi_group_configure(
        ctx: Context<SurroundfiGroupConfigure>,
        new_admin: Pubkey,
        is_arena_group: bool,
    ) -> SurroundfiResult {
        surroundfi_group::configure(ctx, new_admin, is_arena_group)
    }

    pub fn lending_pool_add_bank(
        ctx: Context<LendingPoolAddBank>,
        bank_config: BankConfigCompact,
    ) -> SurroundfiResult {
        surroundfi_group::lending_pool_add_bank(ctx, bank_config)
    }

    /// A copy of lending_pool_add_bank with an additional bank seed.
    /// This seed is used to create a PDA for the bank's signature.
    /// lending_pool_add_bank is preserved for backwards compatibility.
    pub fn lending_pool_add_bank_with_seed(
        ctx: Context<LendingPoolAddBankWithSeed>,
        bank_config: BankConfigCompact,
        bank_seed: u64,
    ) -> SurroundfiResult {
        surroundfi_group::lending_pool_add_bank_with_seed(ctx, bank_config, bank_seed)
    }

    pub fn lending_pool_add_bank_permissionless(
        ctx: Context<LendingPoolAddBankPermissionless>,
        bank_seed: u64,
    ) -> SurroundfiResult {
        surroundfi_group::lending_pool_add_bank_permissionless(ctx, bank_seed)
    }

    pub fn lending_pool_configure_bank(
        ctx: Context<LendingPoolConfigureBank>,
        bank_config_opt: BankConfigOpt,
    ) -> SurroundfiResult {
        surroundfi_group::lending_pool_configure_bank(ctx, bank_config_opt)
    }

    pub fn lending_pool_configure_bank_oracle(
        ctx: Context<LendingPoolConfigureBankOracle>,
        setup: u8,
        oracle: Pubkey,
    ) -> SurroundfiResult {
        surroundfi_group::lending_pool_configure_bank_oracle(ctx, setup, oracle)
    }

    pub fn lending_pool_setup_emissions(
        ctx: Context<LendingPoolSetupEmissions>,
        flags: u64,
        rate: u64,
        total_emissions: u64,
    ) -> SurroundfiResult {
        surroundfi_group::lending_pool_setup_emissions(ctx, flags, rate, total_emissions)
    }

    pub fn lending_pool_update_emissions_parameters(
        ctx: Context<LendingPoolUpdateEmissionsParameters>,
        emissions_flags: Option<u64>,
        emissions_rate: Option<u64>,
        additional_emissions: Option<u64>,
    ) -> SurroundfiResult {
        surroundfi_group::lending_pool_update_emissions_parameters(
            ctx,
            emissions_flags,
            emissions_rate,
            additional_emissions,
        )
    }

    /// Handle bad debt of a bankrupt surroundfi account for a given bank.
    pub fn lending_pool_handle_bankruptcy<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingPoolHandleBankruptcy<'info>>,
    ) -> SurroundfiResult {
        surroundfi_group::lending_pool_handle_bankruptcy(ctx)
    }

    // User instructions

    /// Initialize a surroundfi account for a given group
    pub fn surroundfi_account_initialize(ctx: Context<SurroundfiAccountInitialize>) -> SurroundfiResult {
        surroundfi_account::initialize_account(ctx)
    }

    pub fn lending_account_deposit<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingAccountDeposit<'info>>,
        amount: u64,
        deposit_up_to_limit: Option<bool>,
    ) -> SurroundfiResult {
        surroundfi_account::lending_account_deposit(ctx, amount, deposit_up_to_limit)
    }

    pub fn lending_account_repay<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingAccountRepay<'info>>,
        amount: u64,
        repay_all: Option<bool>,
    ) -> SurroundfiResult {
        surroundfi_account::lending_account_repay(ctx, amount, repay_all)
    }

    pub fn lending_account_withdraw<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingAccountWithdraw<'info>>,
        amount: u64,
        withdraw_all: Option<bool>,
    ) -> SurroundfiResult {
        surroundfi_account::lending_account_withdraw(ctx, amount, withdraw_all)
    }

    pub fn lending_account_borrow<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingAccountBorrow<'info>>,
        amount: u64,
    ) -> SurroundfiResult {
        surroundfi_account::lending_account_borrow(ctx, amount)
    }

    pub fn lending_account_close_balance(
        ctx: Context<LendingAccountCloseBalance>,
    ) -> SurroundfiResult {
        surroundfi_account::lending_account_close_balance(ctx)
    }

    pub fn lending_account_withdraw_emissions<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingAccountWithdrawEmissions<'info>>,
    ) -> SurroundfiResult {
        surroundfi_account::lending_account_withdraw_emissions(ctx)
    }

    pub fn lending_account_settle_emissions(
        ctx: Context<LendingAccountSettleEmissions>,
    ) -> SurroundfiResult {
        surroundfi_account::lending_account_settle_emissions(ctx)
    }

    /// Liquidate a lending account balance of an unhealthy surroundfi account
    pub fn lending_account_liquidate<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingAccountLiquidate<'info>>,
        asset_amount: u64,
    ) -> SurroundfiResult {
        surroundfi_account::lending_account_liquidate(ctx, asset_amount)
    }

    pub fn lending_account_start_flashloan(
        ctx: Context<LendingAccountStartFlashloan>,
        end_index: u64,
    ) -> SurroundfiResult {
        surroundfi_account::lending_account_start_flashloan(ctx, end_index)
    }

    pub fn lending_account_end_flashloan<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingAccountEndFlashloan<'info>>,
    ) -> SurroundfiResult {
        surroundfi_account::lending_account_end_flashloan(ctx)
    }

    pub fn surroundfi_account_update_emissions_destination_account<'info>(
        ctx: Context<'_, '_, 'info, 'info, SurroundfiAccountUpdateEmissionsDestinationAccount<'info>>,
    ) -> SurroundfiResult {
        surroundfi_account::surroundfi_account_update_emissions_destination_account(ctx)
    }

    // Operational instructions
    pub fn lending_pool_accrue_bank_interest(
        ctx: Context<LendingPoolAccrueBankInterest>,
    ) -> SurroundfiResult {
        surroundfi_group::lending_pool_accrue_bank_interest(ctx)
    }

    pub fn lending_pool_collect_bank_fees<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingPoolCollectBankFees<'info>>,
    ) -> SurroundfiResult {
        surroundfi_group::lending_pool_collect_bank_fees(ctx)
    }

    pub fn lending_pool_withdraw_fees<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingPoolWithdrawFees<'info>>,
        amount: u64,
    ) -> SurroundfiResult {
        surroundfi_group::lending_pool_withdraw_fees(ctx, amount)
    }

    pub fn lending_pool_withdraw_insurance<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingPoolWithdrawInsurance<'info>>,
        amount: u64,
    ) -> SurroundfiResult {
        surroundfi_group::lending_pool_withdraw_insurance(ctx, amount)
    }

    pub fn set_account_flag(ctx: Context<SetAccountFlag>, flag: u64) -> SurroundfiResult {
        surroundfi_group::set_account_flag(ctx, flag)
    }

    pub fn unset_account_flag(ctx: Context<UnsetAccountFlag>, flag: u64) -> SurroundfiResult {
        surroundfi_group::unset_account_flag(ctx, flag)
    }

    pub fn set_new_account_authority(
        ctx: Context<SurroundfiAccountSetAccountAuthority>,
    ) -> SurroundfiResult {
        surroundfi_account::set_account_transfer_authority(ctx)
    }

    pub fn surroundfi_account_close(ctx: Context<SurroundfiAccountClose>) -> SurroundfiResult {
        surroundfi_account::close_account(ctx)
    }

    pub fn lending_account_withdraw_emissions_permissionless<'info>(
        ctx: Context<'_, '_, 'info, 'info, LendingAccountWithdrawEmissionsPermissionless<'info>>,
    ) -> SurroundfiResult {
        surroundfi_account::lending_account_withdraw_emissions_permissionless(ctx)
    }

    /// (Permissionless) Refresh the internal risk engine health cache. Useful for liquidators and
    /// other consumers that want to see the internal risk state of a user account. This cache is
    /// read-only and serves no purpose except being populated by this ix.
    /// * remaining accounts expected in the same order as borrow, etc. I.e., for each balance the
    ///   user has, pass bank and oracle: <bank1, oracle1, bank2, oracle2>
    pub fn lending_account_pulse_health<'info>(
        ctx: Context<'_, '_, 'info, 'info, PulseHealth<'info>>,
    ) -> SurroundfiResult {
        surroundfi_account::lending_account_pulse_health(ctx)
    }

    /// (Runs once per program) Configures the fee state account, where the global admin sets fees
    /// that are assessed to the protocol
    pub fn init_global_fee_state(
        ctx: Context<InitFeeState>,
        admin: Pubkey,
        fee_wallet: Pubkey,
        bank_init_flat_sol_fee: u32,
        program_fee_fixed: WrappedI80F48,
        program_fee_rate: WrappedI80F48,
    ) -> SurroundfiResult {
        surroundfi_group::initialize_fee_state(
            ctx,
            admin,
            fee_wallet,
            bank_init_flat_sol_fee,
            program_fee_fixed,
            program_fee_rate,
        )
    }

    /// (global fee admin only) Adjust fees, admin, or the destination wallet
    pub fn edit_global_fee_state(
        ctx: Context<EditFeeState>,
        admin: Pubkey,
        fee_wallet: Pubkey,
        bank_init_flat_sol_fee: u32,
        program_fee_fixed: WrappedI80F48,
        program_fee_rate: WrappedI80F48,
    ) -> SurroundfiResult {
        surroundfi_group::edit_fee_state(
            ctx,
            admin,
            fee_wallet,
            bank_init_flat_sol_fee,
            program_fee_fixed,
            program_fee_rate,
        )
    }

    /// (Permissionless) Force any group to adopt the current FeeState settings
    pub fn propagate_fee_state(ctx: Context<PropagateFee>) -> SurroundfiResult {
        surroundfi_group::propagate_fee(ctx)
    }

    /// (global fee admin only) Enable or disable program fees for any group. Does not require the
    /// group admin to sign: the global fee state admin can turn program fees on or off for any
    /// group
    pub fn config_group_fee(
        ctx: Context<ConfigGroupFee>,
        enable_program_fee: bool,
    ) -> SurroundfiResult {
        surroundfi_group::config_group_fee(ctx, enable_program_fee)
    }

    /// (group admin only) Init the Staked Settings account, which is used to create staked
    /// collateral banks, and must run before any staked collateral bank can be created with
    /// `add_pool_permissionless`. Running this ix effectively opts the group into the staked
    /// collateral feature.
    pub fn init_staked_settings(
        ctx: Context<InitStakedSettings>,
        settings: StakedSettingsConfig,
    ) -> SurroundfiResult {
        surroundfi_group::initialize_staked_settings(ctx, settings)
    }

    pub fn edit_staked_settings(
        ctx: Context<EditStakedSettings>,
        settings: StakedSettingsEditConfig,
    ) -> SurroundfiResult {
        surroundfi_group::edit_staked_settings(ctx, settings)
    }

    pub fn propagate_staked_settings(ctx: Context<PropagateStakedSettings>) -> SurroundfiResult {
        surroundfi_group::propagate_staked_settings(ctx)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
use solana_security_txt::security_txt;
#[cfg(not(feature = "no-entrypoint"))]
security_txt! {
    name: "surroundfi",
    project_url: "https://app.surroundfi.com/",
    contacts: "email:security@surroundfi.com",
    policy: "https://github.com/surround-fi/smart-contracts/blob/main/SECURITY.md",
    preferred_languages: "en",
    source_code: "https://github.com/surround-fi/smart-contracts"
}
