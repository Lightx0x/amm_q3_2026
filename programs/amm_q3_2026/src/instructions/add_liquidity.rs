use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{mint_to, transfer, Mint, MintTo, Token, TokenAccount, Transfer},
};
use constant_product_curve::ConstantProduct;

use crate::{constants::*, error::AmmError, state::Pool};

#[derive(Accounts)]
pub struct AddLiquidity<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    pub mint_a: Account<'info, Mint>,
    pub mint_b: Account<'info, Mint>,
    #[account(
        has_one = mint_a,
        has_one = mint_b,
        seeds = [SEED, pool.seed.to_le_bytes().as_ref()],
        bump = pool.bump,
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
        init_if_needed,
        payer = user,
        associated_token::mint = mint_lp,
        associated_token::authority = user,
    )]
    pub user_lp: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> AddLiquidity<'info> {
    pub fn add_liquidity(&mut self, lp_amount: u64, max_a: u64, max_b: u64) -> Result<()> {
        require!(!self.pool.locked, AmmError::PoolLocked);
        require!(
            lp_amount > 0 && max_a > 0 && max_b > 0,
            AmmError::ZeroAmount
        );

        let (a, b) =
            if self.mint_lp.supply == 0 && self.vault_a.amount == 0 && self.vault_b.amount == 0 {
                (max_a, max_b)
            } else {
                let amounts = ConstantProduct::xy_deposit_amounts_from_l(
                    self.vault_a.amount,
                    self.vault_b.amount,
                    self.mint_lp.supply,
                    lp_amount,
                    PRECISION,
                )
                .map_err(AmmError::from)?;

                require!(
                    amounts.x <= max_a && amounts.y <= max_b,
                    AmmError::SlippageExceeded
                );

                (amounts.x, amounts.y)
            };

        self.deposit_tokens(true, a)?;
        self.deposit_tokens(false, b)?;
        self.mint_lp_tokens(lp_amount)
    }

    fn deposit_tokens(&self, is_a: bool, token_amount: u64) -> Result<()> {
        let (from, to) = match is_a {
            true => (
                self.user_a.to_account_info(),
                self.vault_a.to_account_info(),
            ),
            false => (
                self.user_b.to_account_info(),
                self.vault_b.to_account_info(),
            ),
        };

        let cpi_program = self.token_program.key();

        let cpi_accounts = Transfer {
            from,
            to,
            authority: self.user.to_account_info(),
        };

        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

        transfer(cpi_ctx, token_amount)
    }

    fn mint_lp_tokens(&self, lp_token_amount: u64) -> Result<()> {
        let cpi_program = self.token_program.key();

        let cpi_accounts = MintTo {
            mint: self.mint_lp.to_account_info(),
            to: self.user_lp.to_account_info(),
            authority: self.pool.to_account_info(),
        };

        let signer_seeds: &[&[&[u8]]] =
            &[&[SEED, &self.pool.seed.to_le_bytes(), &[self.pool.bump]]];

        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);
        mint_to(cpi_ctx, lp_token_amount)
    }
}
