use anchor_lang::error_code;
use constant_product_curve::CurveError;

#[error_code]
pub enum AmmError {
    #[msg("LP fee plus protocol fee cannot exceed 100% (10,000 bps).")]
    InvalidFee,
    #[msg("This pool is locked.")]
    PoolLocked,
    #[msg("Only the pool admin can do this.")]
    Unauthorized,
    #[msg("Amount must be greater than zero.")]
    ZeroAmount,
    #[msg("Slippage limit exceeded.")]
    SlippageExceeded,
    #[msg("Pool does not have enough liquidity.")]
    InsufficientLiquidity,
    #[msg("Overflow detected.")]
    Overflow,
    #[msg("Underflow detected.")]
    Underflow,
    #[msg("Invalid precision.")]
    InvalidPrecision,
    #[msg("Insufficient balance.")]
    InsufficientBalance,
    #[msg("Zero balance.")]
    ZeroBalance,
}

impl From<CurveError> for AmmError {
    fn from(error: CurveError) -> AmmError {
        match error {
            CurveError::InvalidPrecision => AmmError::InvalidPrecision,
            CurveError::Overflow => AmmError::Overflow,
            CurveError::Underflow => AmmError::Underflow,
            CurveError::InvalidFeeAmount => AmmError::InvalidFee,
            CurveError::InsufficientBalance => AmmError::InsufficientBalance,
            CurveError::ZeroBalance => AmmError::ZeroBalance,
            CurveError::SlippageLimitExceeded => AmmError::SlippageExceeded,
        }
    }
}
