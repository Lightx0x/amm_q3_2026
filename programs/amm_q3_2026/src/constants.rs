use anchor_lang::prelude::*;

#[constant]
pub const SEED: &[u8] = b"pool";

#[constant]
pub const LP_SEED: &[u8] = b"lp";

#[constant]
pub const BPS: u64 = 10_000;

#[constant]
pub const LP_DECIMALS: u8 = 6;

#[constant]
pub const PRECISION: u32 = 1_000_000;
