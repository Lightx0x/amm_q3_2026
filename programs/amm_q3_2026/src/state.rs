use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Pool {
    pub seed: u64,             // Seed so one program can host many pools
    pub admin: Pubkey,         // Creator; the only key allowed to toggle the lock
    pub treasury: Pubkey,      // Wallet whose ATAs receive the protocol fee
    pub mint_a: Pubkey,        // Token A
    pub mint_b: Pubkey,        // Token B
    pub lp_fee_bps: u16,       // Swap fee kept in the vaults for LPs (basis points)
    pub protocol_fee_bps: u16, // Swap fee forwarded to the treasury (basis points)
    pub locked: bool,          // If true, add_liquidity and swap are refused
    pub pool_bump: u8,         // Bump seed for the pool account
    pub lp_bump: u8,           // Bump seed for the LP mint
}
