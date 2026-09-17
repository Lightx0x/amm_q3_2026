use anchor_lang::prelude::*;
use anchor_spl::token::{transfer, Mint, Token, TokenAccount, Transfer};
use constant_product_curve::{ConstantProduct, LiquidityPair};

use crate::{constants::*, error::AmmError, state::Pool};

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    pub mint_a: Box<Account<'info, Mint>>,
    pub mint_b: Box<Account<'info, Mint>>,
    #[account(
        has_one = mint_a,
        has_one = mint_b,
        seeds = [SEED, pool.seed.to_le_bytes().as_ref()],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,
    #[account(
        seeds = [LP_SEED, pool.key().as_ref()],
        bump = pool.lp_bump,
    )]
    pub mint_lp: Box<Account<'info, Mint>>,
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
        associated_token::mint = mint_a,
        associated_token::authority = pool.treasury,
    )]
    pub treasury_a: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = mint_b,
        associated_token::authority = pool.treasury,
    )]
    pub treasury_b: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}

impl<'info> Swap<'info> {
    pub fn swap(&mut self, is_a: bool, amount_in: u64, min_out: u64) -> Result<()> {
        require!(!self.pool.locked, AmmError::PoolLocked);
        require!(amount_in > 0, AmmError::ZeroAmount);

        // Always hand the curve the input side as X: its Y path prices the trade
        // without applying the fee, which would let every B -> A swap shrink k.
        let (reserve_in, reserve_out) = match is_a {
            true => (self.vault_a.amount, self.vault_b.amount),
            false => (self.vault_b.amount, self.vault_a.amount),
        };

        let mut curve = ConstantProduct::init(
            reserve_in,
            reserve_out,
            self.mint_lp.supply,
            self.pool.lp_fee_bps + self.pool.protocol_fee_bps,
            Some(LP_DECIMALS),
        )
        .map_err(AmmError::from)?;

        let swap_result = curve
            .swap(LiquidityPair::X, amount_in, min_out)
            .map_err(AmmError::from)?;
        require!(swap_result.withdraw > 0, AmmError::InsufficientLiquidity);

        // Protocol share of the fee goes to the treasury; the rest stays in the vault for LPs.
        let protocol_fee =
            (amount_in as u128 * self.pool.protocol_fee_bps as u128 / BPS as u128) as u64;

        self.deposit_tokens(is_a, swap_result.deposit - protocol_fee, protocol_fee)?;
        self.withdraw_tokens(is_a, swap_result.withdraw)
    }

    fn deposit_tokens(&self, is_a: bool, amt_to_vault: u64, amt_to_treasury: u64) -> Result<()> {
        let (from, vault, treasury) = match is_a {
            true => (
                self.user_a.to_account_info(),
                self.vault_a.to_account_info(),
                self.treasury_a.to_account_info(),
            ),
            false => (
                self.user_b.to_account_info(),
                self.vault_b.to_account_info(),
                self.treasury_b.to_account_info(),
            ),
        };

        for (to, amount) in [(vault, amt_to_vault), (treasury, amt_to_treasury)] {
            if amount > 0 {
                transfer(
                    CpiContext::new(
                        self.token_program.key(),
                        Transfer {
                            from: from.clone(),
                            to,
                            authority: self.user.to_account_info(),
                        },
                    ),
                    amount,
                )?;
            }
        }
        Ok(())
    }

    fn withdraw_tokens(&self, is_a: bool, amount: u64) -> Result<()> {
        let (from, to) = match is_a {
            true => (
                self.vault_b.to_account_info(),
                self.user_b.to_account_info(),
            ),
            false => (
                self.vault_a.to_account_info(),
                self.user_a.to_account_info(),
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
                &[&[SEED, &self.pool.seed.to_le_bytes(), &[self.pool.bump]]],
            ),
            amount,
        )
    }
}
