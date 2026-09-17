use crate::{constants::SEED, error::AmmError, state::Pool};
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct ToggleLock<'info> {
    pub admin: Signer<'info>,
    #[account(
        mut,
        has_one = admin @ AmmError::Unauthorized,
        seeds = [SEED, pool.seed.to_le_bytes().as_ref()],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,
}

impl<'info> ToggleLock<'info> {
    pub fn toggle_lock(&mut self) -> Result<()> {
        self.pool.locked = !self.pool.locked;
        Ok(())
    }
}
