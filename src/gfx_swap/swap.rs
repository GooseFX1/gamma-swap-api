use super::GfxSwapClient;
use crate::accounts::{AccountsError, AccountsGetter};
use crate::utils::{derive_authority_pda, derive_pool_pda};

use anchor_lang::prelude::AccountMeta;
use anchor_lang::AccountDeserialize;
use gamma::curve::TradeDirection;
use gamma::states::{AmmConfig, PoolState};
use rand::Rng;
use solana_client::rpc_config::RpcSimulateTransactionConfig;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::hash::Hash;
use solana_sdk::instruction::Instruction;
use solana_sdk::message::{Message, VersionedMessage};
use solana_sdk::signature::Signature;
use solana_sdk::transaction::VersionedTransaction;
use solana_sdk::{pubkey, pubkey::Pubkey};
use swap_api::quote::SwapMode;
use swap_api::swap::{
    SwapInstructionsResponse, SwapInstructionsResponseInternal, SwapRequest, SwapResponse,
};
use swap_api::transaction_config::{
    ComputeUnitPriceMicroLamports, PrioritizationFeeLamports, TransactionConfig,
};
use thiserror::Error;

/// Protocol defined: The default compute units set for a transaction
const DEFAULT_INSTRUCTION_COMPUTE_UNIT: u32 = 200_000;
/// Protocol defined: There are 10^6 micro-lamports in one lamport
const MICRO_LAMPORTS_PER_LAMPORT: u64 = 1_000_000;
/// The cap we set on auto priority-fees
const MAX_AUTO_PRIORITY_FEE_LAMPORTS: u64 = 5_000_000;
/// ID of the referral program
const REFERRAL_PROGRAM_MAINNET: Pubkey = pubkey!("REFER4ZgmyYx9c6He5XfaTMiGfdLwRnkV4RPp9t9iF3");

/// Make sure the cu-price used is at least this value in micro-lamports
const MIN_CU_PRICE: u64 = 20_000;
/// Make sure the priority-fee used is at least this value in lamports
const MIN_PRIORITY_FEE: u64 = 20_000;

#[derive(Debug, Error)]
pub enum SwapError {
    #[error("Error fetching account: {0}")]
    Accounts(#[from] AccountsError),
    #[error("Error deserializing anchor account: {0}")]
    Anchor(#[from] anchor_lang::error::Error),
    #[error("RPC error: {0}")]
    ClientError(#[from] solana_rpc_client_api::client_error::Error),
    #[error("{0}")]
    InvalidRequest(String),
    #[error(transparent)]
    SignerError(#[from] solana_sdk::signer::SignerError),
    #[error(transparent)]
    SerializeTxn(#[from] bincode::Error),
    #[error("Prioritization fee calculation resulted in overflow")]
    PrioritizationFeeOverflow,
}

impl GfxSwapClient {
    pub async fn swap_instructions(
        &self,
        req: &SwapRequest,
    ) -> Result<SwapInstructionsResponseInternal, SwapError> {
        Ok(self.swap_instructions_inner(req).await?.into())
    }

    pub async fn swap_transaction(&self, req: &SwapRequest) -> Result<SwapResponse, SwapError> {
        let blockhash_update = self.blockhash.read().await;
        let instructions = self.swap_instructions_inner(req).await?;
        let transaction = build_transaction(
            instructions,
            Some(&req.user_public_key),
            Some(blockhash_update.hash),
        );

        Ok(SwapResponse {
            swap_transaction: bincode::serialize(&transaction)?,
            last_valid_block_height: blockhash_update.last_valid_block_height,
        })
    }

    async fn swap_instructions_inner(
        &self,
        req: &SwapRequest,
    ) -> Result<SwapInstructionsResponse, SwapError> {
        // Currently ignored:
        // - as-legacy-transaction. Legacy is the default since we only deal with a single swap route
        // - use-shared-accounts
        // - use-token-ledger
        // - fee-account
        // - PrioritizationFeeLamports::Auto, PrioritizationFeeLamports::AutoMultiplier

        if req.quote_response.input_mint == req.quote_response.output_mint {
            return Err(SwapError::InvalidRequest(
                "Input mint cannot equal output mint".to_string(),
            ));
        }

        let TransactionConfig {
            wrap_and_unwrap_sol,
            destination_token_account,
            compute_unit_price_micro_lamports,
            prioritization_fee_lamports,
            dynamic_compute_unit_limit: _,
            as_legacy_transaction: _,
            fee_account: _,
            use_shared_accounts: _,
            use_token_ledger: _,
        } = &req.config;

        let token_ledger_instruction = None;
        let compute_budget_instructions = Vec::new();
        let mut setup_instructions = Vec::new();
        let mut cleanup_instruction = None;

        let token_0_mint = std::cmp::min(
            req.quote_response.input_mint,
            req.quote_response.output_mint,
        );
        let token_1_mint = std::cmp::max(
            req.quote_response.input_mint,
            req.quote_response.output_mint,
        );
        let (pool, _) = derive_pool_pda(
            &self.gamma_config,
            &token_0_mint,
            &token_1_mint,
            &self.gamma_program_id,
        );
        let pool_account = self.accounts_service.get_account(&pool).await?;
        let pool_state = PoolState::try_deserialize(&mut &pool_account[..])?;
        let config_account = self
            .accounts_service
            .get_account(&self.gamma_config)
            .await?;
        let _amm_config = AmmConfig::try_deserialize(&mut &config_account[..])?;

        let trade_direction = if req.quote_response.input_mint == token_0_mint {
            TradeDirection::ZeroForOne
        } else {
            TradeDirection::OneForZero
        };
        let (
            input_vault,
            output_vault,
            input_token_mint,
            output_token_mint,
            input_token_program,
            output_token_program,
        ) = match trade_direction {
            TradeDirection::ZeroForOne => (
                pool_state.token_0_vault,
                pool_state.token_1_vault,
                pool_state.token_0_mint,
                pool_state.token_1_mint,
                pool_state.token_0_program,
                pool_state.token_1_program,
            ),
            TradeDirection::OneForZero => (
                pool_state.token_1_vault,
                pool_state.token_0_vault,
                pool_state.token_1_mint,
                pool_state.token_0_mint,
                pool_state.token_1_program,
                pool_state.token_0_program,
            ),
        };

        let input_ata = spl_associated_token_account::get_associated_token_address_with_program_id(
            &req.user_public_key,
            &req.quote_response.input_mint,
            &input_token_program,
        );
        let output_ata = spl_associated_token_account::get_associated_token_address_with_program_id(
            &req.user_public_key,
            &req.quote_response.output_mint,
            &output_token_program,
        );

        if req.quote_response.input_mint == spl_token::native_mint::ID {
            // Only create an input-ata if it's the native mint
            let create_ata_ix =
                spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                    &req.user_public_key,
                    &req.user_public_key,
                    &req.quote_response.input_mint,
                    &input_token_program,
                );
            setup_instructions.push(create_ata_ix);

            // Only wrap SOL if user specifies this behaviour and the input-token is SOL
            if *wrap_and_unwrap_sol {
                let transfer_ix = solana_sdk::system_instruction::transfer(
                    &req.user_public_key,
                    &input_ata,
                    req.quote_response.in_amount,
                );
                let sync_ix = spl_token::instruction::sync_native(&spl_token::ID, &input_ata)
                    .expect("spl_token::ID is valid");
                setup_instructions.extend([transfer_ix, sync_ix]);

                let close_ix = spl_token_2022::instruction::close_account(
                    &spl_token::ID,
                    &input_ata,
                    &req.user_public_key,
                    &req.user_public_key,
                    &[],
                )
                .expect("spl_token::ID is valid");
                cleanup_instruction = Some(close_ix);
            }
        }

        if destination_token_account.is_none() {
            // Only create an ATA if no destination-token-account is specified. If specified, we assume it is
            // already initialized.
            let create_ata_ix =
                spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                    &req.user_public_key,
                    &req.user_public_key,
                    &req.quote_response.output_mint,
                    &output_token_program,
                );
            setup_instructions.push(create_ata_ix);

            if *wrap_and_unwrap_sol && req.quote_response.output_mint == spl_token::native_mint::ID
            {
                cleanup_instruction = Some(
                    spl_token_2022::instruction::close_account(
                        &output_token_program,
                        &output_ata,
                        &req.user_public_key,
                        &req.user_public_key,
                        &[],
                    )
                    .expect("spl_token::ID is valid"),
                )
            }
        }

        let input_token_account = input_ata;
        let output_token_account = destination_token_account.unwrap_or(output_ata);
        let base_in = match req.quote_response.swap_mode {
            SwapMode::ExactIn => true,
            SwapMode::ExactOut => false,
        };

        let mut accounts = anchor_lang::ToAccountMetas::to_account_metas(
            &gamma::accounts::Swap {
                payer: req.user_public_key,
                authority: derive_authority_pda(&self.gamma_program_id).0,
                amm_config: pool_state.amm_config,
                pool_state: pool,
                input_token_account,
                output_token_account,
                input_vault,
                output_vault,
                input_token_program,
                output_token_program,
                input_token_mint,
                output_token_mint,
                observation_state: pool_state.observation_key,
            },
            None,
        );
        if let Some(referral_account) = self.referral {
            let referral_program = self.referral_program.unwrap_or(REFERRAL_PROGRAM_MAINNET);
            let referral_token_account = Pubkey::find_program_address(
                &[
                    b"referral_ata",
                    referral_account.as_ref(),
                    input_token_mint.as_ref(),
                ],
                &referral_program,
            )
            .0;
            accounts.extend([
                AccountMeta::new_readonly(gamma::ID, false),
                AccountMeta::new_readonly(gamma::ID, false),
                AccountMeta::new_readonly(referral_account, false),
                AccountMeta::new(referral_token_account, false),
            ]);
        }

        let data = if base_in {
            anchor_lang::InstructionData::data(&gamma::instruction::SwapBaseInput {
                amount_in: req.quote_response.in_amount,
                minimum_amount_out: req.quote_response.other_amount_threshold,
            })
        } else {
            anchor_lang::InstructionData::data(&gamma::instruction::SwapBaseOutput {
                max_amount_in: req.quote_response.other_amount_threshold,
                amount_out: req.quote_response.out_amount,
            })
        };
        let swap_instruction = Instruction::new_with_bytes(self.gamma_program_id, &data, accounts);
        let mut instructions = SwapInstructionsResponse {
            token_ledger_instruction,
            compute_budget_instructions,
            setup_instructions,
            swap_instruction,
            cleanup_instruction,
            address_lookup_table_addresses: vec![],
        };

        let dynamic_compute =
            if req.config.dynamic_compute_unit_limit {
                let simulate_txn =
                    build_transaction(instructions.clone(), Some(&req.user_public_key), None);
                let result = self
                    .solana_rpc
                    .simulate_transaction_with_config(
                        &simulate_txn,
                        RpcSimulateTransactionConfig {
                            sig_verify: false,
                            replace_recent_blockhash: true,
                            commitment: Some(CommitmentConfig::confirmed()),
                            ..Default::default()
                        },
                    )
                    .await?;
                result.value.units_consumed.and_then(|compute_units| {
                    u32::try_from(compute_units).ok()?.checked_add(50_000)
                }) // Add 50k more CUs for safety
            } else {
                None
            };

        if let Some(compute_units) = dynamic_compute {
            log::trace!("setting dynamic compute unit: {}", compute_units);
            instructions.compute_budget_instructions.push(
                solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(
                    compute_units,
                ),
            );
        }

        let compute_units = dynamic_compute.unwrap_or(DEFAULT_INSTRUCTION_COMPUTE_UNIT);
        log::debug!("Compute unit price microlamports: {compute_unit_price_micro_lamports:#?}");
        log::debug!("Prioritization fee lamports: {prioritization_fee_lamports:#?}");
        match (
            compute_unit_price_micro_lamports,
            prioritization_fee_lamports,
        ) {
            (Some(ComputeUnitPriceMicroLamports::MicroLamports(cu_price)), _) => {
                log::trace!("setting user defined cu-price: {}", cu_price);
                let compute_ix =
                    solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_price(
                        *cu_price,
                    );
                instructions.compute_budget_instructions.push(compute_ix);
            }
            (Some(ComputeUnitPriceMicroLamports::Auto), _) | (None, None) => {
                let cu_price = match &self.priofees_handle {
                    Some(handle) => std::cmp::max(
                        MIN_CU_PRICE,
                        handle.get_latest_priofee().await.per_compute_unit.medium,
                    ),
                    None => MIN_CU_PRICE,
                };
                log::trace!("cu-price-microlamports: cu-price={}", cu_price);
                let compute_ix =
                    solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_price(
                        cu_price,
                    );
                instructions.compute_budget_instructions.push(compute_ix);
            }
            (None, Some(PrioritizationFeeLamports::Auto)) => {
                // protocol: priority-fee = cu-price * cu-limit / 1_000_000
                // agave: priority-fee = (cu-price * cu-limit + 999_999) / 1_000_000
                let priofee = match &self.priofees_handle {
                    Some(handle) => {
                        let priofee = std::cmp::max(
                            MIN_PRIORITY_FEE,
                            handle.get_latest_priofee().await.per_transaction.medium,
                        );
                        std::cmp::min(priofee, MAX_AUTO_PRIORITY_FEE_LAMPORTS)
                    }
                    None => MAX_AUTO_PRIORITY_FEE_LAMPORTS,
                };
                let cu_price = calculate_cu_price(priofee, compute_units);
                log::trace!(
                    "prioritization-fee-lamports: cu-price={}. priofee={}, cu-limit={}",
                    cu_price,
                    priofee,
                    compute_units
                );
                let compute_ix =
                    solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_price(
                        cu_price,
                    );
                instructions.compute_budget_instructions.push(compute_ix);
            }
            (None, Some(PrioritizationFeeLamports::AutoMultiplier(multiplier))) => {
                let priofee = (*multiplier as u64)
                    .checked_mul(100_000)
                    .ok_or(SwapError::PrioritizationFeeOverflow)?;
                let cu_price = calculate_cu_price(priofee, compute_units);
                log::trace!(
                    "prioritization-fee-lamports: cu-price={}, multiplier={}. priofee={}, cu-limit={}",
                    cu_price,
                    multiplier,
                    priofee,
                    compute_units
                );
                let compute_ix =
                    solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_price(
                        cu_price,
                    );
                instructions.compute_budget_instructions.push(compute_ix);
            }
            (None, Some(PrioritizationFeeLamports::JitoTipLamports(jito_tip))) => {
                let tip_ix = build_jito_tip_ix(&req.user_public_key, *jito_tip);
                instructions.setup_instructions.push(tip_ix);
            }
        }

        Ok(instructions)
    }
}

fn build_transaction(
    instructions: SwapInstructionsResponse,
    payer: Option<&Pubkey>,
    blockhash: Option<Hash>,
) -> VersionedTransaction {
    let mut final_instructions = Vec::new();
    let SwapInstructionsResponse {
        token_ledger_instruction: _,
        compute_budget_instructions,
        setup_instructions,
        swap_instruction,
        cleanup_instruction,
        address_lookup_table_addresses: _,
    } = instructions;
    final_instructions.extend(compute_budget_instructions);
    final_instructions.extend(setup_instructions);
    final_instructions.push(swap_instruction);
    if let Some(cleanup_instruction) = cleanup_instruction {
        final_instructions.push(cleanup_instruction);
    }
    let mut message = VersionedMessage::Legacy(Message::new(&final_instructions, payer));
    if let Some(hash) = blockhash {
        message.set_recent_blockhash(hash);
    }
    VersionedTransaction {
        signatures: vec![Signature::default()],
        message,
    }
}

fn calculate_cu_price(priority_fee: u64, compute_units: u32) -> u64 {
    let cu_price = (priority_fee as u128)
        .checked_mul(MICRO_LAMPORTS_PER_LAMPORT as u128)
        .expect("u128 multiplication shouldn't overflow")
        .saturating_sub(MICRO_LAMPORTS_PER_LAMPORT as u128 - 1)
        .checked_div(compute_units as u128 + 1)
        .expect("non-zero compute units");
    log::trace!("cu-price u128: {}", cu_price);
    u64::try_from(cu_price).unwrap_or(u64::MAX)
}

const JITO_TIP_ACCOUNTS: [Pubkey; 8] = [
    pubkey!("96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5"),
    pubkey!("HFqU5x63VTqvQss8hp11i4wVV8bD44PvwucfZ2bU7gRe"),
    pubkey!("Cw8CFyM9FkoMi7K7Crf6HNQqf4uEMzpKw6QNghXLvLkY"),
    pubkey!("ADaUMid9yfUytqMBgopwjb2DTLSokTSzL1zt6iGPaS49"),
    pubkey!("DfXygSm4jCyNCybVYYK6DwvWqjKee8pbDmJGcLWNDXjh"),
    pubkey!("ADuUkR4vqLUMWXxW9gh6D6L8pMSawimctcNZ5pGwDcEt"),
    pubkey!("DttWaMuVvTiduZRnguLF7jNxTgiMBZ1hyAumKUiL2KRL"),
    pubkey!("3AVi9Tg9Uo68tJfuvoKvqKNWKkC5wPdSSdeBnizKZ6jT"),
];

fn build_jito_tip_ix(from: &Pubkey, tip: u64) -> Instruction {
    let random_recipient =
        &JITO_TIP_ACCOUNTS[rand::thread_rng().gen_range(0..JITO_TIP_ACCOUNTS.len())];
    solana_sdk::system_instruction::transfer(from, random_recipient, tip)
}
