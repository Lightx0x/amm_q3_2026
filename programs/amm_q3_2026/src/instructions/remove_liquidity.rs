use anchor_lang::prelude::*;
use anchor_spl::token::{burn, transfer, Burn, Mint, Token, TokenAccount, Transfer};
use constant_product_curve::ConstantProduct;

use crate::{constants::*, error::AmmError, state::Pool};

#[derive(Accounts)]
pub struct RemoveLiquidity<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    pub mint_a: Account<'info, Mint>,
    pub mint_b: Account<'info, Mint>,
    #[account(
        has_one = mint_a,
        has_one = mint_b,
        seeds = [POOL_SEED, pool.seed.to_le_bytes().as_ref()],
        bump = pool.pool_bump,
    )]
    pub pool: Account<'info, Pool>,
    #[account(
        mut,
        seeds = [LP_SEED, pool.key().as_ref()],
        bump = pool.lp_bump,
    )]
    pub mint_lp: Account<'info, Mint>,
    #[account(
        mut,
        associated_token::mint = mint_a,
        associated_token::authority = pool,
    )]
    pub vault_a: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = mint_b,
        associated_token::authority = pool,
    )]
    pub vault_b: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = mint_a,
        associated_token::authority = user,
    )]
    pub user_a: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = mint_b,
        associated_token::authority = user,
    )]
    pub user_b: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = mint_lp,
        associated_token::authority = user,
    )]
    pub user_lp: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}

impl<'info> RemoveLiquidity<'info> {
    /// Allowed even while the pool is locked so LPs can always exit.
    pub fn remove_liquidity(
        &mut self,
        lp_amount: u64,
        min_amt_a: u64,
        min_amt_b: u64,
    ) -> Result<()> {
        require!(lp_amount > 0, AmmError::ZeroAmount);
        require!(
            lp_amount <= self.mint_lp.supply,
            AmmError::InsufficientBalance
        );

        let amounts = ConstantProduct::xy_withdraw_amounts_from_l(
            self.vault_a.amount,
            self.vault_b.amount,
            self.mint_lp.supply,
            lp_amount,
            PRECISION,
        )
        .map_err(AmmError::from)?;
        let (amt_a, amt_b) = (amounts.x, amounts.y);

        require!(
            amt_a >= min_amt_a && amt_b >= min_amt_b,
            AmmError::SlippageExceeded
        );

        self.burn_lp_tokens(lp_amount)?;
        self.withdraw_tokens(true, amt_a)?;
        self.withdraw_tokens(false, amt_b)
    }

    fn withdraw_tokens(&self, is_a: bool, amount: u64) -> Result<()> {
        let (from, to) = match is_a {
            true => (
                self.vault_a.to_account_info(),
                self.user_a.to_account_info(),
            ),
            false => (
                self.vault_b.to_account_info(),
                self.user_b.to_account_info(),
            ),
        };

        transfer(
            CpiContext::new_with_signer(
                self.token_program.key(),
                Transfer {
                    from,
                    to,
                    authority: self.pool.to_account_info(),
                },
                &[&[
                    POOL_SEED,
                    &self.pool.seed.to_le_bytes(),
                    &[self.pool.pool_bump],
                ]],
            ),
            amount,
        )
    }

    fn burn_lp_tokens(&self, amount: u64) -> Result<()> {
        burn(
            CpiContext::new(
                self.token_program.key(),
                Burn {
                    mint: self.mint_lp.to_account_info(),
                    from: self.user_lp.to_account_info(),
                    authority: self.user.to_account_info(),
                },
            ),
            amount,
        )
    }
}
