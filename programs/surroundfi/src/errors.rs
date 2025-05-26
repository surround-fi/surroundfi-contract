use anchor_lang::prelude::*;

#[error_code]
pub enum SurroundfiError {
    #[msg("Internal Surroundfi logic error")] // 6000
    InternalLogicError,
    #[msg("Invalid bank index")] // 6001
    BankNotFound,
    #[msg("Lending account balance not found")] // 6002
    LendingAccountBalanceNotFound,
    #[msg("Bank deposit capacity exceeded")] // 6003
    BankAssetCapacityExceeded,
    #[msg("Invalid transfer")] // 6004
    InvalidTransfer,
    #[msg("Missing Oracle, Bank, LST mint, or Sol Pool")] // 6005
    MissingPythOrBankAccount,
    #[msg("Missing Pyth account")] // 6006
    MissingPythAccount,
    #[msg("Missing Bank account")] // 6007
    MissingBankAccount,
    #[msg("Invalid Bank account")] // 6008
    InvalidBankAccount,
    #[msg("RiskEngine rejected due to either bad health or stale oracles")] // 6009
    RiskEngineInitRejected,
    #[msg("Lending account balance slots are full")] // 6010
    LendingAccountBalanceSlotsFull,
    #[msg("Bank already exists")] // 6011
    BankAlreadyExists,
    #[msg("Amount to liquidate must be positive")] // 6012
    ZeroLiquidationAmount,
    #[msg("Account is not bankrupt")] // 6013
    AccountNotBankrupt,
    #[msg("Account balance is not bad debt")] // 6014
    BalanceNotBadDebt,
    #[msg("Invalid group config")] // 6015
    InvalidConfig,
    #[msg("Bank paused")] // 6016
    BankPaused,
    #[msg("Bank is ReduceOnly mode")] // 6017
    BankReduceOnly,
    #[msg("Bank is missing")] // 6018
    BankAccountNotFound,
    #[msg("Operation is deposit-only")] // 6019
    OperationDepositOnly,
    #[msg("Operation is withdraw-only")] // 6020
    OperationWithdrawOnly,
    #[msg("Operation is borrow-only")] // 6021
    OperationBorrowOnly,
    #[msg("Operation is repay-only")] // 6022
    OperationRepayOnly,
    #[msg("No asset found")] // 6023
    NoAssetFound,
    #[msg("No liability found")] // 6024
    NoLiabilityFound,
    #[msg("Invalid oracle setup")] // 6025
    InvalidOracleSetup,
    #[msg("Invalid bank utilization ratio")] // 6026
    IllegalUtilizationRatio,
    #[msg("Bank borrow cap exceeded")] // 6027
    BankLiabilityCapacityExceeded,
    #[msg("Invalid Price")] // 6028
    InvalidPrice,
    #[msg("Account can have only one liability when account is under isolated risk")] // 6029
    IsolatedAccountIllegalState,
    #[msg("Emissions already setup")] // 6030
    EmissionsAlreadySetup,
    #[msg("Oracle is not set")] // 6031
    OracleNotSetup,
    #[msg("Invalid switchboard decimal conversion")] // 6032
    InvalidSwitchboardDecimalConversion,
    #[msg("Cannot close balance because of outstanding emissions")] // 6033
    CannotCloseOutstandingEmissions,
    #[msg("Update emissions error")] //6034
    EmissionsUpdateError,
    #[msg("Account disabled")] // 6035
    AccountDisabled,
    #[msg("Account can't temporarily open 3 balances, please close a balance first")] // 6036
    AccountTempActiveBalanceLimitExceeded,
    #[msg("Illegal action during flashloan")] // 6037
    AccountInFlashloan,
    #[msg("Illegal flashloan")] // 6038
    IllegalFlashloan,
    #[msg("Illegal flag")] // 6039
    IllegalFlag,
    #[msg("Illegal balance state")] // 6040
    IllegalBalanceState,
    #[msg("Illegal account authority transfer")] // 6041
    IllegalAccountAuthorityTransfer,
    #[msg("Unauthorized")] // 6042
    Unauthorized,
    #[msg("Invalid account authority")] // 6043
    IllegalAction,
    #[msg("Token22 Banks require mint account as first remaining account")] // 6044
    T22MintRequired,
    #[msg("Invalid ATA for global fee account")] // 6045
    InvalidFeeAta,
    #[msg("Use add pool permissionless instead")] // 6046
    AddedStakedPoolManually,
    #[msg("Staked SOL accounts can only deposit staked assets and borrow SOL")] // 6047
    AssetTagMismatch,
    #[msg("Stake pool validation failed: check the stake pool, mint, or sol pool")] // 6048
    StakePoolValidationFailed,
    #[msg("Switchboard oracle: stale price")] // 6049
    SwitchboardStalePrice,
    #[msg("Pyth Push oracle: stale price")] // 6050
    PythPushStalePrice,
    #[msg("Oracle error: wrong number of accounts")] // 6051
    WrongNumberOfOracleAccounts,
    #[msg("Oracle error: wrong account keys")] // 6052
    WrongOracleAccountKeys,
    #[msg("Pyth Push oracle: wrong account owner")] // 6053
    PythPushWrongAccountOwner,
    #[msg("Staked Pyth Push oracle: wrong account owner")] // 6054
    StakedPythPushWrongAccountOwner,
    #[msg("Pyth Push oracle: mismatched feed id")] // 6055
    PythPushMismatchedFeedId,
    #[msg("Pyth Push oracle: insufficient verification level")] // 6056
    PythPushInsufficientVerificationLevel,
    #[msg("Pyth Push oracle: feed id must be 32 Bytes")] // 6057
    PythPushFeedIdMustBe32Bytes,
    #[msg("Pyth Push oracle: feed id contains non-hex characters")] // 6058
    PythPushFeedIdNonHexCharacter,
    #[msg("Switchboard oracle: wrong account owner")] // 6059
    SwitchboardWrongAccountOwner,
    #[msg("Pyth Push oracle: invalid account")] // 6060
    PythPushInvalidAccount,
    #[msg("Switchboard oracle: invalid account")] // 6061
    SwitchboardInvalidAccount,
    #[msg("Math error")] // 6062
    MathError,
    #[msg("Invalid emissions destination account")] // 6063
    InvalidEmissionsDestinationAccount,
    #[msg("Asset and liability bank cannot be the same")] // 6064
    SameAssetAndLiabilityBanks,
    #[msg("Trying to withdraw more assets than available")] // 6065
    OverliquidationAttempt,
    #[msg("Liability bank has no liabilities")] // 6066
    NoLiabilitiesInLiabilityBank,
    #[msg("Liability bank has assets")] // 6067
    AssetsInLiabilityBank,
    #[msg("Account is healthy and cannot be liquidated")] // 6068
    HealthyAccount,
    #[msg("Liability payoff too severe, exhausted liability")] // 6069
    ExhaustedLiability,
    #[msg("Liability payoff too severe, liability balance has assets")] // 6070
    TooSeverePayoff,
    #[msg("Liquidation too severe, account above maintenance requirement")] // 6071
    TooSevereLiquidation,
    #[msg("Liquidation would worsen account health")] // 6072
    WorseHealthPostLiquidation,
    #[msg("Arena groups can only support two banks")] // 6073
    ArenaBankLimit,
    #[msg("Arena groups cannot return to non-arena status")] // 6074
    ArenaSettingCannotChange,
}

impl From<SurroundfiError> for ProgramError {
    fn from(e: SurroundfiError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl From<pyth_solana_receiver_sdk::error::GetPriceError> for SurroundfiError {
    fn from(e: pyth_solana_receiver_sdk::error::GetPriceError) -> Self {
        match e {
            pyth_solana_receiver_sdk::error::GetPriceError::PriceTooOld => {
                SurroundfiError::PythPushStalePrice
            }
            pyth_solana_receiver_sdk::error::GetPriceError::MismatchedFeedId => {
                SurroundfiError::PythPushMismatchedFeedId
            }
            pyth_solana_receiver_sdk::error::GetPriceError::InsufficientVerificationLevel => {
                SurroundfiError::PythPushInsufficientVerificationLevel
            }
            pyth_solana_receiver_sdk::error::GetPriceError::FeedIdMustBe32Bytes => {
                SurroundfiError::PythPushFeedIdMustBe32Bytes
            }
            pyth_solana_receiver_sdk::error::GetPriceError::FeedIdNonHexCharacter => {
                SurroundfiError::PythPushFeedIdNonHexCharacter
            }
        }
    }
}
impl From<u32> for SurroundfiError {
    fn from(value: u32) -> Self {
        match value {
            6001 => SurroundfiError::BankNotFound,
            6002 => SurroundfiError::LendingAccountBalanceNotFound,
            6003 => SurroundfiError::BankAssetCapacityExceeded,
            6004 => SurroundfiError::InvalidTransfer,
            6005 => SurroundfiError::MissingPythOrBankAccount,
            6006 => SurroundfiError::MissingPythAccount,
            6007 => SurroundfiError::MissingBankAccount,
            6008 => SurroundfiError::InvalidBankAccount,
            6009 => SurroundfiError::RiskEngineInitRejected,
            6010 => SurroundfiError::LendingAccountBalanceSlotsFull,
            6011 => SurroundfiError::BankAlreadyExists,
            6012 => SurroundfiError::ZeroLiquidationAmount,
            6013 => SurroundfiError::AccountNotBankrupt,
            6014 => SurroundfiError::BalanceNotBadDebt,
            6015 => SurroundfiError::InvalidConfig,
            6016 => SurroundfiError::BankPaused,
            6017 => SurroundfiError::BankReduceOnly,
            6018 => SurroundfiError::BankAccountNotFound,
            6019 => SurroundfiError::OperationDepositOnly,
            6020 => SurroundfiError::OperationWithdrawOnly,
            6021 => SurroundfiError::OperationBorrowOnly,
            6022 => SurroundfiError::OperationRepayOnly,
            6023 => SurroundfiError::NoAssetFound,
            6024 => SurroundfiError::NoLiabilityFound,
            6025 => SurroundfiError::InvalidOracleSetup,
            6026 => SurroundfiError::IllegalUtilizationRatio,
            6027 => SurroundfiError::BankLiabilityCapacityExceeded,
            6028 => SurroundfiError::InvalidPrice,
            6029 => SurroundfiError::IsolatedAccountIllegalState,
            6030 => SurroundfiError::EmissionsAlreadySetup,
            6031 => SurroundfiError::OracleNotSetup,
            6032 => SurroundfiError::InvalidSwitchboardDecimalConversion,
            6033 => SurroundfiError::CannotCloseOutstandingEmissions,
            6034 => SurroundfiError::EmissionsUpdateError,
            6035 => SurroundfiError::AccountDisabled,
            6036 => SurroundfiError::AccountTempActiveBalanceLimitExceeded,
            6037 => SurroundfiError::AccountInFlashloan,
            6038 => SurroundfiError::IllegalFlashloan,
            6039 => SurroundfiError::IllegalFlag,
            6040 => SurroundfiError::IllegalBalanceState,
            6041 => SurroundfiError::IllegalAccountAuthorityTransfer,
            6042 => SurroundfiError::Unauthorized,
            6043 => SurroundfiError::IllegalAction,
            6044 => SurroundfiError::T22MintRequired,
            6045 => SurroundfiError::InvalidFeeAta,
            6046 => SurroundfiError::AddedStakedPoolManually,
            6047 => SurroundfiError::AssetTagMismatch,
            6048 => SurroundfiError::StakePoolValidationFailed,
            6049 => SurroundfiError::SwitchboardStalePrice,
            6050 => SurroundfiError::PythPushStalePrice,
            6051 => SurroundfiError::WrongNumberOfOracleAccounts,
            6052 => SurroundfiError::WrongOracleAccountKeys,
            6053 => SurroundfiError::PythPushWrongAccountOwner,
            6054 => SurroundfiError::StakedPythPushWrongAccountOwner,
            6055 => SurroundfiError::PythPushMismatchedFeedId,
            6056 => SurroundfiError::PythPushInsufficientVerificationLevel,
            6057 => SurroundfiError::PythPushFeedIdMustBe32Bytes,
            6058 => SurroundfiError::PythPushFeedIdNonHexCharacter,
            6059 => SurroundfiError::SwitchboardWrongAccountOwner,
            6060 => SurroundfiError::PythPushInvalidAccount,
            6061 => SurroundfiError::SwitchboardInvalidAccount,
            6062 => SurroundfiError::MathError,
            6063 => SurroundfiError::InvalidEmissionsDestinationAccount,
            6064 => SurroundfiError::SameAssetAndLiabilityBanks,
            6065 => SurroundfiError::OverliquidationAttempt,
            6066 => SurroundfiError::NoLiabilitiesInLiabilityBank,
            6067 => SurroundfiError::AssetsInLiabilityBank,
            6068 => SurroundfiError::HealthyAccount,
            6069 => SurroundfiError::ExhaustedLiability,
            6070 => SurroundfiError::TooSeverePayoff,
            6071 => SurroundfiError::TooSevereLiquidation,
            6072 => SurroundfiError::WorseHealthPostLiquidation,
            _ => SurroundfiError::InternalLogicError,
        }
    }
}
