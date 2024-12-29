use crate::accounts::PoolSlice;
use gamma::states::{OBSERVATION_SEED, POOL_SEED, POOL_VAULT_SEED};
use gamma::AUTH_SEED;
use solana_sdk::pubkey::Pubkey;

pub fn derive_pool_pda(
    config: &Pubkey,
    token_0: &Pubkey,
    token_1: &Pubkey,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            POOL_SEED.as_bytes(),
            config.as_ref(),
            token_0.as_ref(),
            token_1.as_ref(),
        ],
        program_id,
    )
}

#[allow(unused)]
pub fn derive_vault_pda(pool: &Pubkey, mint: &Pubkey, program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[POOL_VAULT_SEED.as_bytes(), pool.as_ref(), mint.as_ref()],
        program_id,
    )
}

pub fn derive_observation_pda(pool: &Pubkey, program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[OBSERVATION_SEED.as_bytes(), pool.as_ref()], program_id)
}

pub fn derive_authority_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[AUTH_SEED.as_bytes()], program_id)
}

pub fn get_keys_for_pool_exclusive(
    pool: &Pubkey,
    data: &PoolSlice,
    program_id: &Pubkey,
) -> Vec<Pubkey> {
    vec![
        data.token_0_mint,
        data.token_1_mint,
        derive_observation_pda(pool, program_id).0,
        // derive_vault_pda(pool, &data.token_0_mint, program_id).0,
        // derive_vault_pda(pool, &data.token_1_mint, program_id).0,
    ]
}
