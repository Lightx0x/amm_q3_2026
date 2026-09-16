pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("4ZRnPnXswZz8Ub7VByDzHiEGj7Bq8TzijafufN1tk4Rf");

#[program]
pub mod amm_q3_2026 {
    use super::*;

    pub fn init_pool(
        ctx: Context<InitPool>,
        seed: u64,
        lp_fee_bps: u16,
        protocol_fee_bps: u16,
    ) -> Result<()> {
        ctx.accounts
            .init_pool(seed, lp_fee_bps, protocol_fee_bps, ctx.bumps)
    }

    pub fn add_liquidity(
        ctx: Context<AddLiquidity>,
        lp_amount: u64,
        max_a: u64,
        max_b: u64,
    ) -> Result<()> {
        ctx.accounts.add_liquidity(lp_amount, max_a, max_b)
    }

    pub fn remove_liquidity(
        ctx: Context<RemoveLiquidity>,
        lp_amount: u64,
        min_amt_a: u64,
        min_amt_b: u64,
    ) -> Result<()> {
        ctx.accounts
            .remove_liquidity(lp_amount, min_amt_a, min_amt_b)
    }

    pub fn swap(ctx: Context<Swap>, is_a: bool, amount_in: u64, min_out: u64) -> Result<()> {
        ctx.accounts.swap(is_a, amount_in, min_out)
    }

    pub fn toggle_lock(ctx: Context<ToggleLock>) -> Result<()> {
        ctx.accounts.toggle_lock()
    }
}
