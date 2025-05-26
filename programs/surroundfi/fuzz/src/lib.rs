use std::{
    collections::HashMap,
    mem::size_of,
    ops::AddAssign,
    sync::{Arc, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

use account_state::{AccountInfoCache, AccountsState};
use anchor_lang::{
    accounts::{interface::Interface, interface_account::InterfaceAccount},
    prelude::{AccountInfo, AccountLoader, Context, Program, Pubkey, Rent, Signer, Sysvar},
    Discriminator, Key,
};
use anchor_spl::token_2022::spl_token_2022;
use arbitrary_helpers::{
    AccountIdx, AssetAmount, BankAndOracleConfig, BankIdx, PriceChange, TokenType,
};
use bank_accounts::{get_bank_map, BankAccounts};
use fixed_macro::types::I80F48;
use surroundfi::{
    constants::FEE_STATE_SEED,
    instructions::LendingPoolConfigureBankOracleBumps,
    state::{fee_state::FeeState, surroundfi_group::BankConfigCompact},
};
use surroundfi::{
    errors::SurroundfiError,
    instructions::LendingPoolAddBankBumps,
    prelude::SurroundfiGroup,
    state::{
        surroundfi_account::SurroundfiAccount,
        surroundfi_group::{Bank, BankVaultType, InterestRateConfig},
    },
};
use metrics::{MetricAction, Metrics};
use solana_program::system_program;
use stubs::test_syscall_stubs;
use user_accounts::UserAccount;
use utils::{
    account_info_lifetime_shortener as ails, account_info_ref_lifetime_shortener as airls,
    account_info_slice_lifetime_shortener as aisls,
};

pub mod account_state;
pub mod arbitrary_helpers;
pub mod bank_accounts;
pub mod metrics;
pub mod stubs;
pub mod user_accounts;
pub mod utils;

pub struct SurroundfiFuzzContext<'info> {
    pub surroundfi_group: AccountInfo<'info>,
    pub fee_state: AccountInfo<'info>,
    pub fee_state_wallet: AccountInfo<'info>,
    pub banks: Vec<BankAccounts<'info>>,
    pub surroundfi_accounts: Vec<UserAccount<'info>>,
    pub owner: AccountInfo<'info>,
    pub system_program: AccountInfo<'info>,
    pub rent_sysvar: AccountInfo<'info>,
    pub last_sysvar_current_timestamp: RwLock<u64>,
    pub metrics: Arc<RwLock<Metrics>>,
    pub state: &'info AccountsState,
}

impl<'state> SurroundfiFuzzContext<'state> {
    pub fn setup(
        state: &'state AccountsState,
        bank_configs: &[BankAndOracleConfig],
        n_users: u8,
    ) -> Self {
        let system_program = state.new_program(system_program::id());
        let admin = state.new_sol_account(1_000_000, true, true);
        let fee_state_wallet = state.new_sol_account(1_000_000, true, true);
        let rent_sysvar = state.new_rent_sysvar_account(Rent::free());
        let fee_state = initialize_fee_state(
            state,
            admin.clone(),
            fee_state_wallet.clone(),
            rent_sysvar.clone(),
            system_program.clone(),
        );
        let surroundfi_group = initialize_surroundfi_group(
            state,
            admin.clone(),
            fee_state.clone(),
            system_program.clone(),
        );

        let mut surroundfi_fuzz_context = SurroundfiFuzzContext {
            surroundfi_group,
            fee_state,
            fee_state_wallet,
            banks: vec![],
            owner: admin,
            system_program,
            rent_sysvar,
            surroundfi_accounts: vec![],
            last_sysvar_current_timestamp: RwLock::new(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            ),
            metrics: Arc::new(RwLock::new(Metrics::default())),
            state,
        };
        surroundfi_fuzz_context.advance_time(0);

        bank_configs
            .iter()
            .for_each(|config| surroundfi_fuzz_context.setup_bank(state, Rent::free(), config));

        let token_vec = surroundfi_fuzz_context
            .banks
            .iter()
            .map(|b| b.mint.clone())
            .collect();

        (0..n_users).into_iter().for_each(|_| {
            surroundfi_fuzz_context
                .create_surroundfi_account(state, Rent::free(), &token_vec)
                .unwrap()
        });

        // Create an extra account for seeding the banks
        surroundfi_fuzz_context
            .create_surroundfi_account(state, Rent::free(), &token_vec)
            .unwrap();

        // Seed the banks
        for bank_idx in 0..surroundfi_fuzz_context.banks.len() {
            surroundfi_fuzz_context
                .process_action_deposit(
                    &AccountIdx(surroundfi_fuzz_context.surroundfi_accounts.len() as u8 - 1),
                    &BankIdx(bank_idx as u8),
                    &AssetAmount(
                        1_000
                            * 10_u64
                                .pow(surroundfi_fuzz_context.banks[bank_idx as usize].mint_decimals.into()),
                    ),
                    None,
                )
                .unwrap();
        }

        surroundfi_fuzz_context
    }

    fn get_bank_map<'a>(&'a self) -> HashMap<Pubkey, &'a BankAccounts<'state>> {
        get_bank_map(&self.banks)
    }

    fn refresh_oracle_accounts(&self) {
        self.banks.iter().for_each(|bank| {
            bank.refresh_oracle(
                self.last_sysvar_current_timestamp
                    .read()
                    .unwrap()
                    .to_owned() as i64,
            )
            .unwrap()
        });
    }

    pub fn advance_time(&self, time: u64) {
        self.last_sysvar_current_timestamp
            .write()
            .unwrap()
            .add_assign(time);

        test_syscall_stubs(Some(
            *self.last_sysvar_current_timestamp.read().unwrap() as i64
        ));
    }

    pub fn setup_bank<'a>(
        &'a mut self,
        state: &'state AccountsState,
        rent: Rent,
        initial_bank_config: &BankAndOracleConfig,
    ) {
        log!("Setting up bank with config {:#?}", initial_bank_config);
        let bank = state.new_owned_account(size_of::<Bank>(), surroundfi::id(), rent.clone());

        let mint = state.new_token_mint(
            rent.clone(),
            initial_bank_config.mint_decimals,
            initial_bank_config.token_type,
        );
        let (liquidity_vault_authority, liquidity_vault_authority_bump) =
            state.new_vault_authority(BankVaultType::Liquidity, bank.key);
        let (liquidity_vault, liquidity_vault_bump) = state.new_vault_account(
            BankVaultType::Liquidity,
            mint.clone(),
            liquidity_vault_authority.key,
            bank.key,
        );

        let (insurance_vault_authority, insurance_vault_authority_bump) =
            state.new_vault_authority(BankVaultType::Insurance, bank.key);
        let (insurance_vault, insurance_vault_bump) = state.new_vault_account(
            BankVaultType::Insurance,
            mint.clone(),
            insurance_vault_authority.key,
            bank.key,
        );

        let (fee_vault_authority, fee_vault_authority_bump) =
            state.new_vault_authority(BankVaultType::Fee, bank.key);
        let (fee_vault, fee_vault_bump) = state.new_vault_account(
            BankVaultType::Fee,
            mint.clone(),
            fee_vault_authority.key,
            bank.key,
        );
        let (_fee_state_key, fee_state_bump) =
            Pubkey::find_program_address(&[FEE_STATE_SEED.as_bytes()], &surroundfi::id());

        let oracle = state.new_oracle_account(
            rent.clone(),
            initial_bank_config.oracle_native_price as i64,
            *mint.key,
            initial_bank_config.mint_decimals as i32,
        );

        let add_bank_bumps = LendingPoolAddBankBumps {
            liquidity_vault_authority: liquidity_vault_authority_bump,
            liquidity_vault: liquidity_vault_bump,
            insurance_vault_authority: insurance_vault_authority_bump,
            insurance_vault: insurance_vault_bump,
            fee_vault_authority: fee_vault_authority_bump,
            fee_vault: fee_vault_bump,
            fee_state: fee_state_bump,
        };
        let configure_bumps = LendingPoolConfigureBankOracleBumps {};

        let token_program = match initial_bank_config.token_type {
            TokenType::Tokenkeg => state.new_program(spl_token::id()),
            TokenType::Token22 | TokenType::Token22WithFee { .. } => {
                state.new_program(spl_token_2022::id())
            }
        };

        {
            surroundfi::instructions::surroundfi_group::lending_pool_add_bank(
                Context::new(
                    &surroundfi::ID,
                    &mut surroundfi::instructions::LendingPoolAddBank {
                        surroundfi_group: AccountLoader::try_from(airls(&self.surroundfi_group))
                            .unwrap(),
                        admin: Signer::try_from(airls(&self.owner)).unwrap(),
                        fee_payer: Signer::try_from(airls(&self.owner)).unwrap(),
                        fee_state: AccountLoader::try_from(airls(&self.fee_state)).unwrap(),
                        global_fee_wallet: ails(self.fee_state_wallet.clone()),
                        bank_mint: Box::new(InterfaceAccount::try_from(airls(&mint)).unwrap()),
                        bank: AccountLoader::try_from_unchecked(&surroundfi::ID, airls(&bank))
                            .unwrap(),
                        liquidity_vault_authority: ails(liquidity_vault_authority.clone()),
                        liquidity_vault: Box::new(
                            InterfaceAccount::try_from(airls(&liquidity_vault)).unwrap(),
                        ),
                        insurance_vault_authority: ails(insurance_vault_authority.clone()),
                        insurance_vault: Box::new(
                            InterfaceAccount::try_from(airls(&insurance_vault)).unwrap(),
                        ),
                        fee_vault_authority: ails(fee_vault_authority.clone()),
                        fee_vault: Box::new(InterfaceAccount::try_from(airls(&fee_vault)).unwrap()),
                        rent: Sysvar::from_account_info(airls(&self.rent_sysvar)).unwrap(),
                        token_program: Interface::try_from(airls(&token_program)).unwrap(),
                        system_program: Program::try_from(airls(&self.system_program)).unwrap(),
                    },
                    &[],
                    add_bank_bumps,
                ),
                BankConfigCompact {
                    asset_weight_init: initial_bank_config.asset_weight_init,
                    asset_weight_maint: initial_bank_config.asset_weight_maint,
                    liability_weight_init: initial_bank_config.liability_weight_init,
                    liability_weight_maint: initial_bank_config.liability_weight_maint,
                    deposit_limit: initial_bank_config.deposit_limit,
                    borrow_limit: initial_bank_config.borrow_limit,
                    interest_rate_config: InterestRateConfig {
                        optimal_utilization_rate: I80F48!(0.5).into(),
                        plateau_interest_rate: I80F48!(0.5).into(),
                        max_interest_rate: I80F48!(4).into(),
                        insurance_fee_fixed_apr: I80F48!(0.01).into(),
                        insurance_ir_fee: I80F48!(0.05).into(),
                        protocol_fixed_fee_apr: I80F48!(0.01).into(),
                        protocol_ir_fee: I80F48!(0.1).into(),
                        ..Default::default()
                    }
                    .into(),
                    operational_state:
                        surroundfi::state::surroundfi_group::BankOperationalState::Operational,
                    risk_tier: if !initial_bank_config.risk_tier_isolated {
                        surroundfi::state::surroundfi_group::RiskTier::Collateral
                    } else {
                        surroundfi::state::surroundfi_group::RiskTier::Isolated
                    },
                    oracle_max_age: 100,
                    ..Default::default()
                },
            )
            .unwrap();
        }

        set_discriminator::<Bank>(bank.clone());

        {
            surroundfi::instructions::surroundfi_group::lending_pool_configure_bank_oracle(
                Context::new(
                    &surroundfi::ID,
                    &mut surroundfi::instructions::LendingPoolConfigureBankOracle {
                        group: AccountLoader::try_from(airls(&self.surroundfi_group)).unwrap(),
                        admin: Signer::try_from(airls(&self.owner)).unwrap(),
                        bank: AccountLoader::try_from_unchecked(&surroundfi::ID, airls(&bank))
                            .unwrap(),
                    },
                    &[ails(oracle.clone())],
                    configure_bumps,
                ),
                1,
                oracle.key(),
            )
            .unwrap();
        }

        self.banks.push(BankAccounts {
            bank,
            oracle,
            liquidity_vault,
            insurance_vault,
            fee_vault,
            mint,
            liquidity_vault_authority,
            insurance_vault_authority,
            fee_vault_authority,
            mint_decimals: initial_bank_config.mint_decimals,
            token_program,
        });
    }

    fn create_surroundfi_account<'a>(
        &'a mut self,
        state: &'state AccountsState,
        rent: Rent,
        token_mints: &Vec<AccountInfo<'state>>,
    ) -> anyhow::Result<()> {
        let surroundfi_account =
            state.new_owned_account(size_of::<SurroundfiAccount>(), surroundfi::id(), rent.clone());

        surroundfi::instructions::surroundfi_account::initialize_account(Context::new(
            &surroundfi::id(),
            &mut surroundfi::instructions::surroundfi_account::SurroundfiAccountInitialize {
                surroundfi_group: AccountLoader::try_from(airls(&self.surroundfi_group))?,
                surroundfi_account: AccountLoader::try_from_unchecked(
                    &surroundfi::ID,
                    airls(&surroundfi_account),
                )?,
                authority: Signer::try_from(airls(&self.owner))?,
                fee_payer: Signer::try_from(airls(&self.owner))?,
                system_program: Program::try_from(airls(&self.system_program))?,
            },
            &[],
            Default::default(),
        ))?;

        let token_accounts = token_mints
            .iter()
            .map(|token| {
                state.new_token_account(
                    token.clone(),
                    self.owner.key,
                    100_000_000_000_000_000,
                    rent.clone(),
                )
            })
            .collect();

        set_discriminator::<SurroundfiAccount>(surroundfi_account.clone());

        self.surroundfi_accounts
            .push(UserAccount::new(surroundfi_account, token_accounts));

        Ok(())
    }

    pub fn process_action_deposit(
        &self,
        account_idx: &AccountIdx,
        bank_idx: &BankIdx,
        asset_amount: &AssetAmount,
        deposit_up_to_limit: Option<bool>,
    ) -> anyhow::Result<()> {
        let surroundfi_account = &self.surroundfi_accounts[account_idx.0 as usize];
        sort_balances(airls(&surroundfi_account.surroundfi_account));

        let bank = &self.banks[bank_idx.0 as usize];

        let cache = AccountInfoCache::new(&[
            surroundfi_account.surroundfi_account.clone(),
            bank.bank.clone(),
            surroundfi_account.token_accounts[bank_idx.0 as usize].clone(),
            bank.liquidity_vault.clone(),
        ]);

        let mut remaining_accounts: Vec<AccountInfo> = vec![];
        if bank.token_program.key() == spl_token_2022::ID {
            remaining_accounts.push(ails(bank.mint.clone()));
        }

        let res = surroundfi::instructions::surroundfi_account::lending_account_deposit(
            Context::new(
                &surroundfi::ID,
                &mut surroundfi::instructions::LendingAccountDeposit {
                    group: AccountLoader::try_from(airls(&self.surroundfi_group))?,
                    surroundfi_account: AccountLoader::try_from(airls(
                        &surroundfi_account.surroundfi_account,
                    ))?,
                    authority: Signer::try_from(airls(&self.owner))?,
                    bank: AccountLoader::try_from(airls(&bank.bank))?,
                    signer_token_account: ails(
                        surroundfi_account.token_accounts[bank_idx.0 as usize].clone(),
                    ),
                    liquidity_vault: InterfaceAccount::try_from(airls(
                        &bank.liquidity_vault.clone(),
                    ))?,
                    token_program: Interface::try_from(airls(&bank.token_program))?,
                },
                &remaining_accounts,
                Default::default(),
            ),
            asset_amount.0,
            deposit_up_to_limit,
        );

        let success = if res.is_err() {
            let error = res.unwrap_err();

            self.metrics.write().unwrap().update_error(&error);

            assert!(
                [SurroundfiError::AccountDisabled.into(),].contains(&error),
                "Unexpected deposit error: {:?}",
                error
            );

            cache.revert();

            false
        } else {
            true
        };

        self.metrics
            .write()
            .unwrap()
            .update_metric(MetricAction::Deposit, success);

        Ok(())
    }

    pub fn process_action_repay(
        &self,
        account_idx: &AccountIdx,
        bank_idx: &BankIdx,
        asset_amount: &AssetAmount,
        repay_all: bool,
    ) -> anyhow::Result<()> {
        let surroundfi_account = &self.surroundfi_accounts[account_idx.0 as usize];
        let bank = &self.banks[bank_idx.0 as usize];
        sort_balances(airls(&surroundfi_account.surroundfi_account));

        let cache = AccountInfoCache::new(&[
            surroundfi_account.surroundfi_account.clone(),
            bank.bank.clone(),
            surroundfi_account.token_accounts[bank_idx.0 as usize].clone(),
            bank.liquidity_vault.clone(),
        ]);

        let mut remaining_accounts = vec![];
        if bank.token_program.key() == spl_token_2022::ID {
            remaining_accounts.push(ails(bank.mint.clone()));
        }

        let res = surroundfi::instructions::surroundfi_account::lending_account_repay(
            Context::new(
                &surroundfi::ID,
                &mut surroundfi::instructions::LendingAccountRepay {
                    group: AccountLoader::try_from(airls(&self.surroundfi_group))?,
                    surroundfi_account: AccountLoader::try_from(airls(
                        &surroundfi_account.surroundfi_account,
                    ))?,
                    authority: Signer::try_from(airls(&self.owner))?,
                    bank: AccountLoader::try_from(airls(&bank.bank))?,
                    signer_token_account: ails(
                        surroundfi_account.token_accounts[bank_idx.0 as usize].clone(),
                    ),
                    liquidity_vault: InterfaceAccount::try_from(airls(
                        &bank.liquidity_vault.clone(),
                    ))?,
                    token_program: Interface::try_from(airls(&bank.token_program))?,
                },
                &remaining_accounts,
                Default::default(),
            ),
            asset_amount.0,
            Some(repay_all),
        );

        let success = if res.is_err() {
            let error = res.unwrap_err();

            self.metrics.write().unwrap().update_error(&error);

            assert!(
                vec![
                    SurroundfiError::NoLiabilityFound.into(),
                    SurroundfiError::OperationRepayOnly.into(),
                    // TODO: maybe change
                    SurroundfiError::BankAccountNotFound.into(),
                    SurroundfiError::AccountDisabled.into(),
                ]
                .contains(&error),
                "Unexpected repay error: {:?}",
                error
            );

            cache.revert();

            false
        } else {
            true
        };

        self.metrics
            .write()
            .unwrap()
            .update_metric(MetricAction::Repay, success);

        Ok(())
    }

    pub fn process_action_withdraw(
        &'state self,
        account_idx: &AccountIdx,
        bank_idx: &BankIdx,
        asset_amount: &AssetAmount,
        withdraw_all: Option<bool>,
    ) -> anyhow::Result<()> {
        self.refresh_oracle_accounts();
        let surroundfi_account = &self.surroundfi_accounts[account_idx.0 as usize];
        sort_balances(airls(&surroundfi_account.surroundfi_account));

        let bank = &self.banks[bank_idx.0 as usize];

        let cache = AccountInfoCache::new(&[
            surroundfi_account.surroundfi_account.clone(),
            bank.bank.clone(),
            surroundfi_account.token_accounts[bank_idx.0 as usize].clone(),
            bank.liquidity_vault.clone(),
        ]);

        let remove_all_bank = if let Some(withdraw_all) = withdraw_all {
            if withdraw_all {
                vec![bank.bank.key()]
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        let mut remaining_accounts = vec![];
        if bank.token_program.key() == spl_token_2022::ID {
            remaining_accounts.push(ails(bank.mint.clone()));
        }
        remaining_accounts.extend(surroundfi_account.get_remaining_accounts(
            &self.get_bank_map(),
            vec![],
            remove_all_bank,
        ));
        let res = surroundfi::instructions::surroundfi_account::lending_account_withdraw(
            Context::new(
                &surroundfi::ID,
                &mut surroundfi::instructions::LendingAccountWithdraw {
                    group: AccountLoader::try_from(airls(&self.surroundfi_group))?,
                    surroundfi_account: AccountLoader::try_from(airls(
                        &surroundfi_account.surroundfi_account,
                    ))?,
                    authority: Signer::try_from(airls(&self.owner))?,
                    bank: AccountLoader::try_from(airls(&bank.bank))?,
                    token_program: Interface::try_from(airls(&bank.token_program))?,
                    destination_token_account: InterfaceAccount::try_from(airls(
                        &surroundfi_account.token_accounts[bank_idx.0 as usize],
                    ))?,
                    bank_liquidity_vault_authority: ails(bank.liquidity_vault_authority.clone()),
                    liquidity_vault: InterfaceAccount::try_from(airls(&bank.liquidity_vault))?,
                },
                aisls(&remaining_accounts),
                Default::default(),
            ),
            asset_amount.0,
            withdraw_all,
        );

        let success = if res.is_err() {
            let error = res.unwrap_err();

            self.metrics.write().unwrap().update_error(&error);

            assert!(
                [
                    SurroundfiError::OperationWithdrawOnly.into(),
                    SurroundfiError::IllegalUtilizationRatio.into(),
                    SurroundfiError::RiskEngineInitRejected.into(),
                    SurroundfiError::NoAssetFound.into(),
                    SurroundfiError::BankAccountNotFound.into(),
                    SurroundfiError::AccountDisabled.into(),
                ]
                .contains(&error),
                "Unexpected withdraw error: {:?}",
                error
            );

            cache.revert();

            false
        } else {
            true
        };

        self.metrics
            .write()
            .unwrap()
            .update_metric(MetricAction::Withdraw, success);

        Ok(())
    }

    pub fn process_action_borrow(
        &'state self,
        account_idx: &AccountIdx,
        bank_idx: &BankIdx,
        asset_amount: &AssetAmount,
    ) -> anyhow::Result<()> {
        self.refresh_oracle_accounts();

        let surroundfi_account = &self.surroundfi_accounts[account_idx.0 as usize];
        let bank = &self.banks[bank_idx.0 as usize];
        let cache = AccountInfoCache::new(&[
            surroundfi_account.surroundfi_account.clone(),
            bank.bank.clone(),
            surroundfi_account.token_accounts[bank_idx.0 as usize].clone(),
            bank.liquidity_vault.clone(),
        ]);
        sort_balances(airls(&surroundfi_account.surroundfi_account));

        let mut remaining_accounts = vec![];
        if bank.token_program.key() == spl_token_2022::ID {
            remaining_accounts.push(ails(bank.mint.clone()));
        }
        remaining_accounts.extend(surroundfi_account.get_remaining_accounts(
            &self.get_bank_map(),
            vec![bank.bank.key()],
            vec![],
        ));
        let res = surroundfi::instructions::surroundfi_account::lending_account_borrow(
            Context::new(
                &surroundfi::ID,
                &mut surroundfi::instructions::LendingAccountBorrow {
                    group: AccountLoader::try_from(airls(&self.surroundfi_group))?,
                    surroundfi_account: AccountLoader::try_from(airls(
                        &surroundfi_account.surroundfi_account,
                    ))?,
                    authority: Signer::try_from(airls(&self.owner))?,
                    bank: AccountLoader::try_from(airls(&bank.bank))?,
                    token_program: Interface::try_from(airls(&bank.token_program))?,
                    destination_token_account: InterfaceAccount::try_from(airls(
                        &surroundfi_account.token_accounts[bank_idx.0 as usize],
                    ))?,
                    bank_liquidity_vault_authority: ails(bank.liquidity_vault_authority.clone()),
                    liquidity_vault: InterfaceAccount::try_from(airls(&bank.liquidity_vault))?,
                },
                aisls(&remaining_accounts),
                Default::default(),
            ),
            asset_amount.0,
        );

        let success = if res.is_err() {
            let error = res.unwrap_err();

            self.metrics.write().unwrap().update_error(&error);

            assert!(
                vec![
                    SurroundfiError::RiskEngineInitRejected.into(),
                    SurroundfiError::IsolatedAccountIllegalState.into(),
                    SurroundfiError::IllegalUtilizationRatio.into(),
                    SurroundfiError::AccountDisabled.into(),
                ]
                .contains(&error),
                "Unexpected borrow error: {:?}",
                error
            );

            cache.revert();

            false
        } else {
            true
        };

        self.metrics
            .write()
            .unwrap()
            .update_metric(MetricAction::Borrow, success);

        Ok(())
    }

    pub fn process_liquidate_account(
        &'state self,
        liquidator_idx: &AccountIdx,
        liquidatee_idx: &AccountIdx,
        asset_amount: &AssetAmount,
    ) -> anyhow::Result<()> {
        self.refresh_oracle_accounts();
        let liquidator_account = &self.surroundfi_accounts[liquidator_idx.0 as usize];
        let liquidatee_account = &self.surroundfi_accounts[liquidatee_idx.0 as usize];
        sort_balances(airls(&liquidator_account.surroundfi_account));
        sort_balances(airls(&liquidatee_account.surroundfi_account));

        if liquidator_account.surroundfi_account.key() == liquidatee_account.surroundfi_account.key() {
            self.metrics
                .write()
                .unwrap()
                .update_metric(MetricAction::Liquidate, false);

            return Ok(());
        }

        let (asset_bank_idx, liab_bank_idx) =
            if let Some(a) = liquidatee_account.get_liquidation_banks(&self.banks) {
                if a.0 == a.1 {
                    self.metrics
                        .write()
                        .unwrap()
                        .update_metric(MetricAction::Liquidate, false);

                    return Ok(());
                } else {
                    a
                }
            } else {
                self.metrics
                    .write()
                    .unwrap()
                    .update_metric(MetricAction::Liquidate, false);

                return Ok(());
            };

        let asset_bank = &self.banks[asset_bank_idx.0 as usize];
        let liab_bank = &self.banks[liab_bank_idx.0 as usize];

        let account_cache = AccountInfoCache::new(&[
            liquidator_account.margin_account.clone(),
            liquidatee_account.margin_account.clone(),
            asset_bank.bank.clone(),
            asset_bank.liquidity_vault.clone(),
            liab_bank.bank.clone(),
            liab_bank.liquidity_vault.clone(),
            liab_bank.insurance_vault.clone(),
        ]);

        let mut remaining_accounts = vec![];
        if liab_bank.token_program.key() == spl_token_2022::ID {
            remaining_accounts.push(ails(liab_bank.mint.clone()));
        }
        remaining_accounts.extend(vec![asset_bank.oracle.clone(), liab_bank.oracle.clone()]);

        let mut liquidator_remaining_accounts = liquidator_account.get_remaining_accounts(
            &self.get_bank_map(),
            vec![liab_bank.bank.key(), asset_bank.bank.key()],
            vec![],
        );
        let mut liquidatee_remaining_accounts =
            liquidatee_account.get_remaining_accounts(&self.get_bank_map(), vec![], vec![]);

        remaining_accounts.append(&mut liquidator_remaining_accounts);
        remaining_accounts.append(&mut liquidatee_remaining_accounts);

        let res = surroundfi::instructions::lending_account_liquidate(
            Context::new(
                &surroundfi::id(),
                &mut surroundfi::instructions::LendingAccountLiquidate {
                    group: AccountLoader::try_from(airls(&self.surroundfi_group))?,
                    asset_bank: AccountLoader::try_from(airls(&asset_bank.bank))?,
                    liab_bank: AccountLoader::try_from(airls(&liab_bank.bank))?,
                    liquidator_surroundfi_account: AccountLoader::try_from(airls(
                        &liquidator_account.surroundfi_account,
                    ))?,
                    authority: Signer::try_from(airls(&self.owner))?,
                    liquidatee_surroundfi_account: AccountLoader::try_from(airls(
                        &liquidatee_account.surroundfi_account,
                    ))?,
                    bank_liquidity_vault_authority: ails(
                        liab_bank.liquidity_vault_authority.clone(),
                    ),
                    bank_liquidity_vault: Box::new(InterfaceAccount::try_from(airls(
                        &liab_bank.liquidity_vault,
                    ))?),
                    bank_insurance_vault: ails(liab_bank.insurance_vault.clone()),
                    token_program: Interface::try_from(airls(&liab_bank.token_program))?,
                },
                aisls(&remaining_accounts),
                Default::default(),
            ),
            asset_amount.0,
        );

        let success = if res.is_err() {
            let error = res.unwrap_err();

            self.metrics.write().unwrap().update_error(&error);

            assert!(
                vec![
                    SurroundfiError::RiskEngineInitRejected.into(),
                    SurroundfiError::IsolatedAccountIllegalState.into(),
                    SurroundfiError::IllegalUtilizationRatio.into(),
                    SurroundfiError::ZeroLiquidationAmount.into(),
                    SurroundfiError::OverliquidationAttempt.into(),
                    SurroundfiError::HealthyAccount.into(),
                    SurroundfiError::ExhaustedLiability.into(),
                    SurroundfiError::TooSevereLiquidation.into(),
                    SurroundfiError::AccountDisabled.into(),
                    SurroundfiError::MathError.into(), // TODO: would be best to avoid this one
                ]
                .contains(&error),
                "Unexpected liquidate error: {:?}",
                error
            );

            account_cache.revert();

            false
        } else {
            self.process_handle_bankruptcy(liquidatee_idx, &liab_bank_idx)?;

            true
        };

        self.metrics
            .write()
            .unwrap()
            .update_metric(MetricAction::Liquidate, success);

        Ok(())
    }

    pub fn process_handle_bankruptcy(
        &'state self,
        account_idx: &AccountIdx,
        bank_idx: &BankIdx,
    ) -> anyhow::Result<()> {
        log!("Action: Handle Bankruptcy");

        let surroundfi_account = &self.surroundfi_accounts[account_idx.0 as usize];
        let bank = &self.banks[bank_idx.0 as usize];

        let cache = AccountInfoCache::new(&[
            bank.bank.clone(),
            surroundfi_account.surroundfi_account.clone(),
            bank.liquidity_vault.clone(),
            bank.insurance_vault.clone(),
        ]);

        let mut remaining_accounts = vec![];
        if bank.token_program.key() == spl_token_2022::ID {
            remaining_accounts.push(ails(bank.mint.clone()));
        }
        remaining_accounts.extend(surroundfi_account.get_remaining_accounts(
            &self.get_bank_map(),
            vec![],
            vec![],
        ));
        let res = surroundfi::instructions::lending_pool_handle_bankruptcy(Context::new(
            &surroundfi::ID,
            &mut surroundfi::instructions::LendingPoolHandleBankruptcy {
                group: AccountLoader::try_from(airls(&self.surroundfi_group))?,
                signer: Signer::try_from(airls(&self.owner))?,
                bank: AccountLoader::try_from(airls(&bank.bank))?,
                surroundfi_account: AccountLoader::try_from(airls(&surroundfi_account.surroundfi_account))?,
                liquidity_vault: ails(bank.liquidity_vault.clone()),
                insurance_vault: Box::new(InterfaceAccount::try_from(airls(
                    &bank.insurance_vault,
                ))?),
                insurance_vault_authority: ails(bank.insurance_vault_authority.clone()),
                token_program: Interface::try_from(airls(&bank.token_program))?,
            },
            aisls(&remaining_accounts),
            Default::default(),
        ));

        let success = if res.is_err() {
            let error = res.unwrap_err();

            self.metrics.write().unwrap().update_error(&error);

            assert!(
                vec![
                    SurroundfiError::AccountDisabled.into(),
                    SurroundfiError::AccountNotBankrupt.into(),
                ]
                .contains(&error),
                "Unexpected handle bankruptcy error: {:?}",
                error
            );

            cache.revert();

            false
        } else {
            true
        };

        self.metrics
            .write()
            .unwrap()
            .update_metric(MetricAction::Bankruptcy, success);

        Ok(())
    }

    pub fn process_update_oracle(
        &self,
        bank_idx: &BankIdx,
        price_change: &PriceChange,
    ) -> anyhow::Result<()> {
        log!("Action: Update Oracle");
        let bank = &self.banks[bank_idx.0 as usize];

        bank.update_oracle(price_change.0)?;

        self.metrics.write().unwrap().price_update += 1;

        Ok(())
    }
}

fn sort_balances<'a>(surroundfi_account_ai: &'a AccountInfo<'a>) {
    let surroundfi_account_loader =
        AccountLoader::<SurroundfiAccount>::try_from(surroundfi_account_ai).unwrap();
    let mut surroundfi_account = surroundfi_account_loader.load_mut().unwrap();
    surroundfi_account
        .lending_account
        .balances
        .sort_by_key(|a| !a.active);
}

pub fn set_discriminator<T: Discriminator>(ai: AccountInfo) {
    let mut data = ai.try_borrow_mut_data().unwrap();

    if data[..8].ne(&[0u8; 8]) {
        panic!("Account discriminator is already set");
    }

    data[..8].copy_from_slice(&T::DISCRIMINATOR);
}

fn initialize_surroundfi_group<'a>(
    state: &'a AccountsState,
    admin: AccountInfo<'a>,
    fee_state: AccountInfo<'a>,
    system_program: AccountInfo<'a>,
) -> AccountInfo<'a> {
    let program_id = surroundfi::id();
    let surroundfi_group =
        state.new_owned_account(size_of::<SurroundfiGroup>(), program_id, Rent::free());

    surroundfi::instructions::surroundfi_group::initialize_group(
        Context::new(
            &surroundfi::id(),
            &mut surroundfi::instructions::SurroundfiGroupInitialize {
                // Unchecked because we are initializing the account.
                surroundfi_group: AccountLoader::try_from_unchecked(
                    &program_id,
                    airls(&surroundfi_group),
                )
                .unwrap(),
                admin: Signer::try_from(airls(&admin)).unwrap(),
                fee_state: AccountLoader::try_from_unchecked(&program_id, airls(&fee_state))
                    .unwrap(),
                system_program: Program::try_from(airls(&system_program)).unwrap(),
            },
            &[],
            Default::default(),
        ),
        false,
    )
    .unwrap();

    set_discriminator::<SurroundfiGroup>(surroundfi_group.clone());

    surroundfi_group
}

fn initialize_fee_state<'a>(
    state: &'a AccountsState,
    admin: AccountInfo<'a>,
    wallet: AccountInfo<'a>,
    rent: AccountInfo<'a>,
    system_program: AccountInfo<'a>,
) -> AccountInfo<'a> {
    let program_id = surroundfi::id();
    let (fee_state, _fee_state_bump) = state.new_fee_state(program_id);

    surroundfi::instructions::surroundfi_group::initialize_fee_state(
        Context::new(
            &surroundfi::id(),
            &mut surroundfi::instructions::InitFeeState {
                payer: Signer::try_from(airls(&admin)).unwrap(),
                fee_state: AccountLoader::try_from_unchecked(&program_id, airls(&fee_state))
                    .unwrap(),
                rent: Sysvar::from_account_info(airls(&rent)).unwrap(),
                system_program: Program::try_from(airls(&system_program)).unwrap(),
            },
            &[],
            Default::default(),
        ),
        admin.key(),
        wallet.key(),
        // WARN: tests will fail at add_bank::system_program::transfer if this is non-zero because
        // the fuzz suite does not yet support the system program.
        0,
        I80F48!(0).into(),
        I80F48!(0).into(),
    )
    .unwrap();

    set_discriminator::<FeeState>(fee_state.clone());

    fee_state
}

#[cfg(test)]
mod tests {
    use fixed::types::I80F48;
    use surroundfi::state::surroundfi_account::RiskEngine;
    use pyth_sdk_solana::state::PriceAccount;

    use super::*;
    #[test]
    fn deposit_test() {
        let account_state = AccountsState::new();

        let a = SurroundfiFuzzContext::setup(&account_state, &[BankAndOracleConfig::dummy(); 2], 2);

        let al =
            AccountLoader::<SurroundfiGroup>::try_from_unchecked(&surroundfi::id(), &a.surroundfi_group)
                .unwrap();

        assert_eq!(al.load().unwrap().admin, a.owner.key());

        a.process_action_deposit(&AccountIdx(0), &BankIdx(0), &AssetAmount(1000), None)
            .unwrap();

        let surroundfi_account_ai = AccountLoader::<SurroundfiAccount>::try_from_unchecked(
            &surroundfi::id(),
            &a.surroundfi_accounts[0].surroundfi_account,
        )
        .unwrap();
        let surroundfi_account = surroundfi_account_ai.load().unwrap();

        assert_eq!(
            I80F48::from(surroundfi_account.lending_account.balances[0].asset_shares),
            I80F48!(1000)
        );
    }

    #[test]
    fn borrow_test() {
        let account_state = AccountsState::new();
        let a = SurroundfiFuzzContext::setup(&account_state, &[BankAndOracleConfig::dummy(); 2], 2);

        a.process_action_deposit(&AccountIdx(1), &BankIdx(1), &AssetAmount(1000), None)
            .unwrap();
        a.process_action_deposit(&AccountIdx(0), &BankIdx(0), &AssetAmount(1000), None)
            .unwrap();
        a.process_action_borrow(&AccountIdx(0), &BankIdx(1), &AssetAmount(100))
            .unwrap();

        let surroundfi_account_ai = AccountLoader::<SurroundfiAccount>::try_from_unchecked(
            &surroundfi::id(),
            &a.surroundfi_accounts[0].surroundfi_account,
        )
        .unwrap();

        {
            let surroundfi_account = surroundfi_account_ai.load().unwrap();

            assert_eq!(
                I80F48::from(surroundfi_account.lending_account.balances[0].asset_shares),
                I80F48!(1000)
            );
            assert_eq!(
                I80F48::from(surroundfi_account.lending_account.balances[1].liability_shares),
                I80F48!(100)
            );
        }

        a.process_action_repay(&AccountIdx(0), &BankIdx(1), &AssetAmount(100), false)
            .unwrap();

        let surroundfi_account = surroundfi_account_ai.load().unwrap();

        assert_eq!(
            I80F48::from(surroundfi_account.lending_account.balances[1].liability_shares),
            I80F48!(0)
        );
    }

    #[test]
    fn liquidation_test() {
        let account_state = AccountsState::new();
        let a = SurroundfiFuzzContext::setup(&account_state, &[BankAndOracleConfig::dummy(); 2], 3);

        a.process_action_deposit(&AccountIdx(1), &BankIdx(1), &AssetAmount(1000), None)
            .unwrap();
        a.process_action_deposit(&AccountIdx(0), &BankIdx(0), &AssetAmount(1000), None)
            .unwrap();
        a.process_action_borrow(&AccountIdx(0), &BankIdx(1), &AssetAmount(500))
            .unwrap();

        a.banks[1].log_oracle_price().unwrap();

        a.process_update_oracle(&BankIdx(1), &PriceChange(10000000000000))
            .unwrap();

        a.banks[1].log_oracle_price().unwrap();

        let surroundfi_account_ai = AccountLoader::<SurroundfiAccount>::try_from_unchecked(
            &surroundfi::id(),
            &a.surroundfi_accounts[0].surroundfi_account,
        )
        .unwrap();

        {
            let surroundfi_account = surroundfi_account_ai.load().unwrap();
            let bank_map = a.get_bank_map();
            let remaining_accounts =
                surroundfi_account.get_remaining_accounts(&bank_map, vec![], vec![]);

            let re = RiskEngine::new(&surroundfi_account, aisls(&remaining_accounts)).unwrap();

            let (assets, liabs) = re
                .get_account_health_components(
                    surroundfi::state::surroundfi_account::RiskRequirementType::Maintenance,
                    &mut None,
                )
                .unwrap();

            println!("assets {assets} liabs: {liabs}");
        }

        a.process_action_deposit(&AccountIdx(2), &BankIdx(1), &AssetAmount(1000), None)
            .unwrap();

        a.process_liquidate_account(&AccountIdx(2), &AccountIdx(0), &AssetAmount(50))
            .unwrap();

        let surroundfi_account_ai = AccountLoader::<SurroundfiAccount>::try_from_unchecked(
            &surroundfi::id(),
            &a.surroundfi_accounts[0].surroundfi_account,
        )
        .unwrap();

        let surroundfi_account = surroundfi_account_ai.load().unwrap();

        assert_eq!(
            I80F48::from(surroundfi_account.lending_account.balances[0].asset_shares),
            I80F48!(950)
        );
    }

    #[test]
    fn liquidation_and_bankruptcy() {
        let account_state = AccountsState::new();

        let a = SurroundfiFuzzContext::setup(&account_state, &[BankAndOracleConfig::dummy(); 2], 3);

        a.process_action_deposit(&AccountIdx(1), &BankIdx(1), &AssetAmount(1000), None)
            .unwrap();
        a.process_action_deposit(&AccountIdx(0), &BankIdx(0), &AssetAmount(1000), None)
            .unwrap();
        a.process_action_borrow(&AccountIdx(0), &BankIdx(1), &AssetAmount(500))
            .unwrap();

        a.process_update_oracle(&BankIdx(1), &PriceChange(1000000000000))
            .unwrap();

        let surroundfi_account_ai = AccountLoader::<SurroundfiAccount>::try_from_unchecked(
            &surroundfi::id(),
            &a.surroundfi_accounts[0].surroundfi_account,
        )
        .unwrap();

        {
            let surroundfi_account = surroundfi_account_ai.load().unwrap();
            let bank_map = a.get_bank_map();
            let remaining_accounts =
                surroundfi_account.get_remaining_accounts(&bank_map, vec![], vec![]);

            let re = RiskEngine::new(&surroundfi_account, aisls(&remaining_accounts)).unwrap();

            let (assets, liabs) = re
                .get_account_health_components(
                    surroundfi::state::surroundfi_account::RiskRequirementType::Maintenance,
                    &mut None,
                )
                .unwrap();

            println!("assets {assets} liabs: {liabs}");
        }

        a.process_action_deposit(&AccountIdx(2), &BankIdx(1), &AssetAmount(1000), None)
            .unwrap();

        a.process_liquidate_account(&AccountIdx(2), &AccountIdx(0), &AssetAmount(1000))
            .unwrap();

        let surroundfi_account_ai = AccountLoader::<SurroundfiAccount>::try_from_unchecked(
            &surroundfi::id(),
            &a.surroundfi_accounts[0].surroundfi_account,
        )
        .unwrap();

        let surroundfi_account = surroundfi_account_ai.load().unwrap();

        assert_eq!(
            I80F48::from(surroundfi_account.lending_account.balances[0].asset_shares),
            I80F48!(0)
        );
        assert_eq!(
            I80F48::from(surroundfi_account.lending_account.balances[0].liability_shares),
            I80F48!(0)
        );
    }

    #[test]
    fn price_update() {
        let account_state = AccountsState::new();

        let a = SurroundfiFuzzContext::setup(&account_state, &[BankAndOracleConfig::dummy(); 2], 3);

        a.process_update_oracle(&BankIdx(0), &PriceChange(1100))
            .unwrap();

        let new_price = {
            let data = a.banks[0].oracle.try_borrow_data().unwrap();
            let data = bytemuck::from_bytes::<PriceAccount>(&data);
            data.ema_price.val
        };

        assert_eq!(new_price, 1100);
    }

    #[test]
    fn pyth_timestamp_update() {
        let account_state = AccountsState::new();

        let a = SurroundfiFuzzContext::setup(&account_state, &[BankAndOracleConfig::dummy(); 2], 3);

        let initial_timestamp = {
            let data = a.banks[0].oracle.try_borrow_data().unwrap();
            let data = bytemuck::from_bytes::<PriceAccount>(&data);
            data.timestamp
        };
        assert_eq!(initial_timestamp, 0);

        a.banks[0].refresh_oracle(123_456).unwrap();

        let updated_timestamp_via_0_10 = {
            let pf =
                pyth_sdk_solana::load_price_feed_from_account_info(&a.banks[0].oracle).unwrap();

            pf.get_ema_price_unchecked().publish_time
        };
        assert_eq!(updated_timestamp_via_0_10, 123_456);
    }
}
