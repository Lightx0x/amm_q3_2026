use {
    amm_q3_2026::{accounts, error::AmmError, instruction as ix, state::Pool, LP_SEED, SEED},
    anchor_lang::{system_program, AccountDeserialize, InstructionData, ToAccountMetas},
    anchor_spl::associated_token::{self, get_associated_token_address as ata},
    litesvm::{types::TransactionResult, LiteSVM},
    litesvm_token::{
        get_spl_account,
        spl_token::{
            self,
            state::{Account as TokenAccount, Mint},
        },
        CreateAssociatedTokenAccount, CreateMint, MintTo,
    },
    solana_keypair::Keypair,
    solana_message::{Instruction, Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

const IX_SEED: u64 = 7;
const LP_FEE_BPS: u16 = 25;
const PROTOCOL_FEE_BPS: u16 = 5;

const WALLET: u64 = 1_000_000_000; // what the user holds of each token
const POOL_A: u64 = 100_000_000; // first deposit: 100 A
const POOL_B: u64 = 400_000_000; // first deposit: 400 B, so the pool is 1:4
const POOL_LP: u64 = 200_000_000; // LP minted for that first deposit

/// A pool plus one user who is also the admin, and every address involved.
struct Amm {
    svm: LiteSVM,
    user: Keypair,
    treasury: Pubkey,
    mint_a: Pubkey,
    mint_b: Pubkey,
    pool: Pubkey,
    mint_lp: Pubkey,
    vault_a: Pubkey,
    vault_b: Pubkey,
    treasury_a: Pubkey,
    treasury_b: Pubkey,
    user_a: Pubkey,
    user_b: Pubkey,
    user_lp: Pubkey,
}

impl Amm {
    /// A funded user and every address the program will touch. No pool yet.
    fn setup() -> Self {
        let mut svm = LiteSVM::new();
        svm.add_program(
            amm_q3_2026::id(),
            include_bytes!("../../../target/deploy/amm_q3_2026.so"),
        )
        .unwrap();

        let user = Keypair::new();
        let owner = user.pubkey();
        svm.airdrop(&owner, 10_000_000_000).unwrap();

        let mut new_mint = || {
            CreateMint::new(&mut svm, &user)
                .decimals(6)
                .authority(&owner)
                .send()
                .unwrap()
        };
        let (mint_a, mint_b) = (new_mint(), new_mint());

        let mut fund = |mint: &Pubkey| {
            let account = CreateAssociatedTokenAccount::new(&mut svm, &user, mint)
                .owner(&owner)
                .send()
                .unwrap();
            MintTo::new(&mut svm, &user, mint, &account, WALLET)
                .send()
                .unwrap();
            account
        };
        let (user_a, user_b) = (fund(&mint_a), fund(&mint_b));

        let treasury = Pubkey::new_unique();
        let pool =
            Pubkey::find_program_address(&[SEED, &IX_SEED.to_le_bytes()], &amm_q3_2026::id()).0;
        let mint_lp = Pubkey::find_program_address(&[LP_SEED, pool.as_ref()], &amm_q3_2026::id()).0;

        Self {
            svm,
            user,
            treasury,
            mint_a,
            mint_b,
            pool,
            mint_lp,
            vault_a: ata(&pool, &mint_a),
            vault_b: ata(&pool, &mint_b),
            treasury_a: ata(&treasury, &mint_a),
            treasury_b: ata(&treasury, &mint_b),
            user_a,
            user_b,
            user_lp: ata(&owner, &mint_lp),
        }
    }

    /// Same as `setup`, with the pool created.
    fn setup_pool() -> Self {
        let mut amm = Self::setup();
        amm.run(&[amm.init_pool(LP_FEE_BPS, PROTOCOL_FEE_BPS)]);
        amm
    }

    /// Same as `setup_pool`, with the first deposit already made.
    fn setup_funded() -> Self {
        let mut amm = Self::setup_pool();
        amm.run(&[amm.add_liquidity(POOL_LP, POOL_A, POOL_B)]);
        amm
    }

    fn owner(&self) -> Pubkey {
        self.user.pubkey()
    }

    // ---- sending -----------------------------------------------------------

    fn send(&mut self, ixs: &[Instruction], signer: &Keypair) -> TransactionResult {
        self.svm.expire_blockhash();
        let payer = self.owner();
        let msg = Message::new_with_blockhash(ixs, Some(&payer), &self.svm.latest_blockhash());
        let signers: Vec<&Keypair> = if signer.pubkey() == payer {
            vec![&self.user]
        } else {
            vec![&self.user, signer]
        };
        let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &signers).unwrap();
        self.svm.send_transaction(tx)
    }

    /// Send and require success.
    fn run(&mut self, ixs: &[Instruction]) {
        let signer = self.user.insecure_clone();
        if let Err(e) = self.send(ixs, &signer) {
            panic!(
                "expected success, got {:?}\n{}",
                e.err,
                e.meta.pretty_logs()
            );
        }
    }

    /// Send and require this exact program error.
    fn expect_err(&mut self, ix: Instruction, want: AmmError) {
        let signer = self.user.insecure_clone();
        let failed = self
            .send(&[ix], &signer)
            .expect_err("expected this to fail, but it succeeded");
        let code = format!("Custom({})", u32::from(want));
        assert!(
            format!("{:?}", failed.err).contains(&code),
            "expected {code}, got {:?}\n{}",
            failed.err,
            failed.meta.pretty_logs()
        );
    }

    // ---- instruction builders ----------------------------------------------

    fn build(&self, data: impl InstructionData, accts: impl ToAccountMetas) -> Instruction {
        Instruction::new_with_bytes(
            amm_q3_2026::id(),
            &data.data(),
            accts.to_account_metas(None),
        )
    }

    fn init_pool(&self, lp_fee_bps: u16, protocol_fee_bps: u16) -> Instruction {
        self.build(
            ix::InitPool {
                seed: IX_SEED,
                lp_fee_bps,
                protocol_fee_bps,
            },
            accounts::InitPool {
                admin: self.owner(),
                mint_a: self.mint_a,
                mint_b: self.mint_b,
                treasury: self.treasury,
                pool: self.pool,
                mint_lp: self.mint_lp,
                vault_a: self.vault_a,
                vault_b: self.vault_b,
                treasury_a: self.treasury_a,
                treasury_b: self.treasury_b,
                token_program: spl_token::ID,
                associated_token_program: associated_token::ID,
                system_program: system_program::ID,
            },
        )
    }

    fn add_liquidity(&self, lp_amount: u64, max_a: u64, max_b: u64) -> Instruction {
        self.build(
            ix::AddLiquidity {
                lp_amount,
                max_a,
                max_b,
            },
            accounts::AddLiquidity {
                user: self.owner(),
                mint_a: self.mint_a,
                mint_b: self.mint_b,
                pool: self.pool,
                mint_lp: self.mint_lp,
                vault_a: self.vault_a,
                vault_b: self.vault_b,
                user_a: self.user_a,
                user_b: self.user_b,
                user_lp: self.user_lp,
                token_program: spl_token::ID,
                associated_token_program: associated_token::ID,
                system_program: system_program::ID,
            },
        )
    }

    fn remove_liquidity(&self, lp_amount: u64, min_amt_a: u64, min_amt_b: u64) -> Instruction {
        self.build(
            ix::RemoveLiquidity {
                lp_amount,
                min_amt_a,
                min_amt_b,
            },
            accounts::RemoveLiquidity {
                user: self.owner(),
                mint_a: self.mint_a,
                mint_b: self.mint_b,
                pool: self.pool,
                mint_lp: self.mint_lp,
                vault_a: self.vault_a,
                vault_b: self.vault_b,
                user_a: self.user_a,
                user_b: self.user_b,
                user_lp: self.user_lp,
                token_program: spl_token::ID,
            },
        )
    }

    fn swap(&self, is_a: bool, amount_in: u64, min_out: u64) -> Instruction {
        self.build(
            ix::Swap {
                is_a,
                amount_in,
                min_out,
            },
            accounts::Swap {
                user: self.owner(),
                mint_a: self.mint_a,
                mint_b: self.mint_b,
                pool: self.pool,
                mint_lp: self.mint_lp,
                vault_a: self.vault_a,
                vault_b: self.vault_b,
                user_a: self.user_a,
                user_b: self.user_b,
                treasury_a: self.treasury_a,
                treasury_b: self.treasury_b,
                token_program: spl_token::ID,
            },
        )
    }

    fn toggle_lock(&self, admin: Pubkey) -> Instruction {
        self.build(
            ix::ToggleLock {},
            accounts::ToggleLock {
                admin,
                pool: self.pool,
            },
        )
    }

    // ---- reading state -----------------------------------------------------

    fn balance(&self, token_account: &Pubkey) -> u64 {
        get_spl_account::<TokenAccount>(&self.svm, token_account)
            .map(|a| a.amount)
            .unwrap_or(0)
    }

    fn lp_supply(&self) -> u64 {
        get_spl_account::<Mint>(&self.svm, &self.mint_lp)
            .unwrap()
            .supply
    }

    fn pool(&self) -> Pool {
        let data = self.svm.get_account(&self.pool).unwrap().data;
        Pool::try_deserialize(&mut data.as_slice()).unwrap()
    }

    /// The constant product the curve must never let shrink.
    fn k(&self) -> u128 {
        self.balance(&self.vault_a) as u128 * self.balance(&self.vault_b) as u128
    }
}

// ===========================================================================
// 1. init_pool
// ===========================================================================

#[test]
fn init_pool_sets_up_the_pool() {
    let mut amm = Amm::setup();
    amm.run(&[amm.init_pool(LP_FEE_BPS, PROTOCOL_FEE_BPS)]);

    let pool = amm.pool();
    assert_eq!(pool.admin, amm.owner());
    assert_eq!(pool.treasury, amm.treasury);
    assert_eq!(pool.mint_a, amm.mint_a);
    assert_eq!(pool.mint_b, amm.mint_b);
    assert_eq!(pool.lp_fee_bps, LP_FEE_BPS);
    assert_eq!(pool.protocol_fee_bps, PROTOCOL_FEE_BPS);
    assert!(!pool.locked);

    // The LP mint is empty and owned by the pool; all four token accounts exist.
    let lp = get_spl_account::<Mint>(&amm.svm, &amm.mint_lp).unwrap();
    assert_eq!(lp.supply, 0);
    assert_eq!(lp.mint_authority.unwrap(), amm.pool);
    for account in [amm.vault_a, amm.vault_b, amm.treasury_a, amm.treasury_b] {
        assert!(
            amm.balance(&account) == 0,
            "{account} should exist and be empty"
        );
    }
}

// ===========================================================================
// 2. add_liquidity
// ===========================================================================

#[test]
fn add_liquidity_takes_tokens_and_mints_lp() {
    let mut amm = Amm::setup_funded();

    // The first deposit sets the ratio: it takes the maximums as given.
    assert_eq!(amm.balance(&amm.vault_a), POOL_A);
    assert_eq!(amm.balance(&amm.vault_b), POOL_B);
    assert_eq!(amm.balance(&amm.user_lp), POOL_LP);
    assert_eq!(amm.lp_supply(), POOL_LP);

    // The pool is 1:4 with 200 LP, so 100 more LP must cost exactly 50 A and 200 B.
    amm.run(&[amm.add_liquidity(100_000_000, 50_000_000, 200_000_000)]);
    assert_eq!(amm.balance(&amm.vault_a), 150_000_000);
    assert_eq!(amm.balance(&amm.vault_b), 600_000_000);
    assert_eq!(amm.lp_supply(), 300_000_000);
}

// ===========================================================================
// 3. remove_liquidity
// ===========================================================================

#[test]
fn remove_liquidity_burns_lp_and_returns_tokens() {
    let mut amm = Amm::setup_funded();

    // Burn half the LP supply, get half of each vault back.
    amm.run(&[amm.remove_liquidity(POOL_LP / 2, POOL_A / 2, POOL_B / 2)]);

    assert_eq!(amm.lp_supply(), POOL_LP / 2);
    assert_eq!(amm.balance(&amm.vault_a), POOL_A / 2);
    assert_eq!(amm.balance(&amm.vault_b), POOL_B / 2);
    assert_eq!(amm.balance(&amm.user_a), WALLET - POOL_A / 2);
    assert_eq!(amm.balance(&amm.user_b), WALLET - POOL_B / 2);
}

// ===========================================================================
// 4. swap
// ===========================================================================

#[test]
fn swap_works_both_ways_and_pays_the_treasury() {
    let mut amm = Amm::setup_funded();
    let amount_in = 10_000_000;
    let protocol_fee = amount_in * PROTOCOL_FEE_BPS as u64 / 10_000;

    // A -> B: the user spends A and receives B.
    let (user_b_before, k_before) = (amm.balance(&amm.user_b), amm.k());
    amm.run(&[amm.swap(true, amount_in, 1)]);
    assert_eq!(amm.balance(&amm.user_a), WALLET - POOL_A - amount_in);
    assert!(amm.balance(&amm.user_b) > user_b_before);
    assert_eq!(amm.balance(&amm.treasury_a), protocol_fee); // protocol fee, in the input token
    assert!(amm.k() > k_before, "the LP fee must grow k");

    // B -> A: the mirror image, paying the fee in B this time.
    let (user_a_before, k_before) = (amm.balance(&amm.user_a), amm.k());
    amm.run(&[amm.swap(false, amount_in, 1)]);
    assert!(amm.balance(&amm.user_a) > user_a_before);
    assert_eq!(amm.balance(&amm.treasury_b), protocol_fee);
    assert!(amm.k() > k_before, "the LP fee must grow k");
}

// ===========================================================================
// 5. toggle_lock, happy path
// ===========================================================================

#[test]
fn toggle_lock_flips_the_lock_and_gates_the_pool() {
    let mut amm = Amm::setup_funded();
    assert!(!amm.pool().locked);

    // Lock it: deposits and swaps stop, but LPs can still withdraw.
    amm.run(&[amm.toggle_lock(amm.owner())]);
    assert!(amm.pool().locked);
    amm.expect_err(
        amm.add_liquidity(1_000_000, 1_000_000, 4_000_000),
        AmmError::PoolLocked,
    );
    amm.expect_err(amm.swap(true, 1_000_000, 1), AmmError::PoolLocked);
    amm.run(&[amm.remove_liquidity(POOL_LP / 2, 1, 1)]);

    // Toggle again: the same instruction unlocks it and the pool works again.
    amm.run(&[amm.toggle_lock(amm.owner())]);
    assert!(!amm.pool().locked);
    amm.run(&[amm.swap(true, 1_000_000, 1)]);
}

// ===========================================================================
// 6. toggle_lock, error path
// ===========================================================================

#[test]
fn toggle_lock_rejects_anyone_but_the_admin() {
    let mut amm = Amm::setup_funded();
    let stranger = Keypair::new();
    amm.svm.airdrop(&stranger.pubkey(), 1_000_000_000).unwrap();

    let ix = amm.toggle_lock(stranger.pubkey());
    let failed = amm
        .send(&[ix], &stranger)
        .expect_err("a stranger must not be able to lock the pool");
    let code = format!("Custom({})", u32::from(AmmError::Unauthorized));
    assert!(
        format!("{:?}", failed.err).contains(&code),
        "expected {code}, got {:?}",
        failed.err
    );

    assert!(!amm.pool().locked, "the pool must still be unlocked");
}

// ===========================================================================
// 7. bad input on every other instruction
// ===========================================================================

#[test]
fn instructions_reject_bad_input() {
    let mut amm = Amm::setup_funded();

    // Fees adding up to more than 100% are refused, on a pool not yet created.
    let mut fresh = Amm::setup();
    let bad_fees = fresh.init_pool(9_000, 1_001);
    fresh.expect_err(bad_fees, AmmError::InvalidFee);

    // Zero amounts are refused.
    amm.expect_err(amm.add_liquidity(0, POOL_A, POOL_B), AmmError::ZeroAmount);
    amm.expect_err(amm.remove_liquidity(0, 0, 0), AmmError::ZeroAmount);
    amm.expect_err(amm.swap(true, 0, 0), AmmError::ZeroAmount);

    // 100 LP costs exactly 50 A; one lamport less than that is not enough.
    amm.expect_err(
        amm.add_liquidity(100_000_000, 49_999_999, 200_000_000),
        AmmError::SlippageExceeded,
    );

    // Nobody can burn more LP than exists, or demand an impossible payout.
    amm.expect_err(
        amm.remove_liquidity(POOL_LP + 1, 0, 0),
        AmmError::InsufficientBalance,
    );
    amm.expect_err(
        amm.swap(true, 1_000_000, u64::MAX),
        AmmError::SlippageExceeded,
    );
}
