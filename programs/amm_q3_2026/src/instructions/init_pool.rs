use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount},
};

use crate::{constants::*, error::AmmError, state::Pool};

#[derive(Accounts)]
#[instruction(seed: u64)]
pub struct InitPool<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,
    pub mint_a: Account<'info, Mint>,
    pub mint_b: Account<'info, Mint>,
    /// CHECK: any wallet; it only ever receives fee tokens through its ATAs.
    pub treasury: UncheckedAccount<'info>,
    #[account(
        init,
        payer = admin,
        seeds = [POOL_SEED, seed.to_le_bytes().as_ref()],
        bump,
        space = Pool::DISCRIMINATOR.len() + Pool::INIT_SPACE,
    )]
    pub pool: Account<'info, Pool>,
    #[account(
        init,
        payer = admin,
        seeds = [LP_SEED, pool.key().as_ref()],
        bump,
        mint::decimals = LP_DECIMALS,
        mint::authority = pool,
    )]
    pub mint_lp: Account<'info, Mint>,
    #[account(
        init,
        payer = admin,
        associated_token::mint = mint_a,
        associated_token::authority = pool,
    )]
    pub vault_a: Box<Account<'info, TokenAccount>>,
    #[account(
        init,
        payer = admin,
        associated_token::mint = mint_b,
        associated_token::authority = pool,
    )]
    pub vault_b: Box<Account<'info, TokenAccount>>,
    #[account(
        init,
        payer = admin,
        associated_token::mint = mint_a,
        associated_token::authority = treasury,
    )]
    pub treasury_a: Box<Account<'info, TokenAccount>>,
    #[account(
        init,
        payer = admin,
        associated_token::mint = mint_b,
        associated_token::authority = treasury,
    )]
    pub treasury_b: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> InitPool<'info> {
    pub fn init_pool(
        &mut self,
        seed: u64,
        lp_fee_bps: u16,
        protocol_fee_bps: u16,
        bumps: InitPoolBumps,
    ) -> Result<()> {
        require!(
            lp_fee_bps as u64 + protocol_fee_bps as u64 <= BPS,
            AmmError::InvalidFee
        );

        self.pool.set_inner(Pool {
            seed,
            admin: self.admin.key(),
            treasury: self.treasury.key(),
            mint_a: self.mint_a.key(),
            mint_b: self.mint_b.key(),
            lp_fee_bps,
            protocol_fee_bps,
            locked: false,
            pool_bump: bumps.pool,
            lp_bump: bumps.mint_lp,
        });

        Ok(())
    }
}
