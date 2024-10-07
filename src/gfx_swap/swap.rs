use super::GfxSwapClient;
use crate::accounts::{AccountsError, AccountsGetter};
use crate::utils::{derive_authority_pda, derive_pool_pda};

use anchor_lang::AccountDeserialize;
use gamma::curve::TradeDirection;
use gamma::states::PoolState;
use jupiter_swap_api_client::quote::SwapMode;
use jupiter_swap_api_client::swap::{
    SwapInstructionsResponse, SwapInstructionsResponseInternal, SwapRequest, SwapResponse,
};
use jupiter_swap_api_client::transaction_config::{
    ComputeUnitPriceMicroLamports, PrioritizationFeeLamports, TransactionConfig,
};
use rand::Rng;
use solana_client::rpc_config::RpcSimulateTransactionConfig;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::hash::Hash;
use solana_sdk::instruction::Instruction;
use solana_sdk::message::{Message, VersionedMessage};
use solana_sdk::transaction::VersionedTransaction;
use solana_sdk::{pubkey, pubkey::Pubkey};
use thiserror::Error;

pub const MAX_COMPUTE_UNIT_LIMIT: u32 = 140_000;

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
}

impl GfxSwapClient {
    pub async fn swap_instructions(
        &self,
        req: &SwapRequest,
    ) -> Result<SwapInstructionsResponseInternal, SwapError> {
        Ok(self.swap_instructions_inner(req).await?.into())
    }

    pub async fn swap_transaction(&self, req: &SwapRequest) -> Result<SwapResponse, SwapError> {
        let payer = &req.user_public_key;
        let guard = self.blockhash.read().await;
        let final_txn =
            build_transaction(self.swap_instructions_inner(req).await?, guard.hash, payer);
        let swap_transaction = bincode::serialize(&final_txn)?;

        Ok(SwapResponse {
            swap_transaction,
            last_valid_block_height: guard.last_valid_block_height,
        })
    }

    async fn swap_instructions_inner(
        &self,
        req: &SwapRequest,
    ) -> Result<SwapInstructionsResponse, SwapError> {
        // Currently ignored:
        // - as-legacy-transaction. Since we only deal with a single route, txns are always legacy and use no lookup-tables
        // - use-shared-accounts
        // - use-token-ledger
        // - fee-account
        // - ComputeUnitMicroLamports::Auto
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
        let mut compute_budget_instructions = Vec::new();
        let mut setup_instructions = Vec::new();
        let mut cleanup_instruction = None;

        match (
            compute_unit_price_micro_lamports,
            prioritization_fee_lamports,
        ) {
            (Some(ComputeUnitPriceMicroLamports::MicroLamports(cu_lamports)), _) => {
                let compute_ix =
                    solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_price(
                        *cu_lamports,
                    );
                compute_budget_instructions.push(compute_ix);
            }
            (None, Some(PrioritizationFeeLamports::JitoTipLamports(jito_tip))) => {
                let tip_ix = build_jito_tip_ix(&req.user_public_key, *jito_tip);
                setup_instructions.push(tip_ix);
            }
            _ => {}
        }

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
                let sync_ix =
                    spl_token::instruction::sync_native(&spl_token::ID, &input_ata).unwrap();
                setup_instructions.extend([transfer_ix, sync_ix]);
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

            if *wrap_and_unwrap_sol {
                let close_params = if req.quote_response.output_mint == spl_token::native_mint::ID {
                    // Close output ata if destination-token-account was not specified and the output-mint is SOL
                    Some((output_ata, output_token_program))
                } else if req.quote_response.input_mint == spl_token::native_mint::ID {
                    // Close the SOL ata since it was previously created
                    Some((input_ata, input_token_program))
                } else {
                    None
                };

                if let Some((account, token_program)) = close_params {
                    cleanup_instruction = Some(
                        spl_token_2022::instruction::close_account(
                            &token_program,
                            &account,
                            &req.user_public_key,
                            &req.user_public_key,
                            &[],
                        )
                        .unwrap(),
                    )
                }
            }
        }

        let input_token_account = input_ata;
        let output_token_account = destination_token_account.unwrap_or(output_ata);
        let base_in = match req.quote_response.swap_mode {
            SwapMode::ExactIn => true,
            SwapMode::ExactOut => false,
        };

        let accounts = anchor_lang::ToAccountMetas::to_account_metas(
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

        let dynamic_compute = if req.config.dynamic_compute_unit_limit {
            let simulate_txn = build_transaction(
                instructions.clone(),
                self.blockhash.read().await.hash,
                &req.user_public_key,
            );
            let result = self
                .solana_rpc
                .simulate_transaction_with_config(
                    &simulate_txn,
                    RpcSimulateTransactionConfig {
                        sig_verify: false,
                        replace_recent_blockhash: false,
                        commitment: Some(CommitmentConfig::confirmed()),
                        ..Default::default()
                    },
                )
                .await?;
            result.value.units_consumed.and_then(|compute_units| u32::try_from(compute_units).ok()?.checked_add(50_000)) // Add 50k more CUs for safety
        } else {
            None
        };
        let compute_units = dynamic_compute.unwrap_or(MAX_COMPUTE_UNIT_LIMIT);
        instructions.compute_budget_instructions.push(
            solana_sdk::compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(
                compute_units,
            ),
        );

        Ok(instructions)
    }
}

fn build_transaction(
    instructions: SwapInstructionsResponse,
    blockhash: Hash,
    payer: &Pubkey,
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
    let mut message = VersionedMessage::Legacy(Message::new(&final_instructions, Some(payer)));
    message.set_recent_blockhash(blockhash);
    VersionedTransaction {
        signatures: vec![],
        message,
    }
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
