use crate::accounts::{AccountsError, AccountsGetter};
use crate::gfx_swap::GfxSwapClient;
use crate::utils::derive_pool_pda;

use std::ops::{Div, Mul, Sub};
use std::time::{Instant, SystemTime};

use anchor_lang::AccountDeserialize;
use gamma::curve::{CurveCalculator, TradeDirection};
use gamma::states::{AmmConfig, ObservationState, PoolState};
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use solana_sdk::program_error::ProgramError;
use spl_token_2022::{
    extension::transfer_fee::{TransferFeeConfig, MAX_FEE_BASIS_POINTS},
    extension::{BaseState, BaseStateWithExtensions, StateWithExtensionsMut},
    state::Mint,
};
use swap_api::quote::{QuoteRequest, QuoteResponse, SwapMode};
use swap_api::route_plan_with_metadata::{RoutePlanStep, SwapInfo};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum QuoteError {
    #[error("Error fetching account: {0}")]
    Accounts(#[from] AccountsError),
    #[error("Error deserializing account: {0}")]
    Unpack(#[from] ProgramError),
    #[error("Error deserializing anchor account: {0}")]
    Anchor(#[from] anchor_lang::error::Error),
    #[error("RPC error: {0}")]
    ClientError(#[from] solana_rpc_client_api::client_error::Error),
    #[error("{0}")]
    InvalidRequest(String),
    #[error("No pool exists for this input-mint - output-mint pair")]
    PairNotTradeable,
    #[error("{0}")]
    Any(#[from] anyhow::Error),
}

impl GfxSwapClient {
    pub async fn quote(&self, quote: &QuoteRequest) -> Result<QuoteResponse, QuoteError> {
        let start = Instant::now();
        let epoch_info = self.solana_rpc.get_epoch_info().await?;
        let min_context_slot = epoch_info.absolute_slot;
        let epoch = epoch_info.epoch;

        if quote.input_mint == quote.output_mint {
            return Err(QuoteError::InvalidRequest(
                "Input mint cannot equal output mint".to_string(),
            ));
        }

        let token_0_mint = std::cmp::min(quote.input_mint, quote.output_mint);
        let token_1_mint = std::cmp::max(quote.input_mint, quote.output_mint);
        let (pool, _) = derive_pool_pda(
            &self.gamma_config,
            &token_0_mint,
            &token_1_mint,
            &self.gamma_program_id,
        );

        let pool_account = self
            .accounts_service
            .get_account(&pool)
            .await
            .map_err(|_| QuoteError::PairNotTradeable)?;
        let pool_state = PoolState::try_deserialize(&mut &pool_account[..])?;
        let observation = pool_state.observation_key;

        let amm_config_account = self
            .accounts_service
            .get_account(&self.gamma_config)
            .await?;
        let mut token_0_mint_account = self.accounts_service.get_account(&token_0_mint).await?;
        let mut token_1_mint_account = self.accounts_service.get_account(&token_1_mint).await?;
        let observation_account = self.accounts_service.get_account(&observation).await?;

        let amm_config = AmmConfig::try_deserialize(&mut &amm_config_account[..])?;
        let observation_state = ObservationState::try_deserialize(&mut &observation_account[..])?;
        let token_0_mint_info = StateWithExtensionsMut::<Mint>::unpack(&mut token_0_mint_account)?;
        let token_1_mint_info = StateWithExtensionsMut::<Mint>::unpack(&mut token_1_mint_account)?;

        let swap_mode = quote.swap_mode.clone().unwrap_or_default();
        let base_in = match swap_mode {
            SwapMode::ExactIn => true,
            SwapMode::ExactOut => false,
        };

        let token_0_vault_amount = pool_state.token_0_vault_amount;
        let token_1_vault_amount = pool_state.token_1_vault_amount;
        log::debug!("Pool: {}", pool);
        log::debug!("Token0 vault amount: {}", token_0_vault_amount);
        log::debug!("Token1 vault amount: {}", token_1_vault_amount);
        let (total_token_0_amount, total_token_1_amount) = pool_state.vault_amount_without_fee()?;

        let (
            _trade_direction,
            total_input_token_amount,
            total_output_token_amount,
            input_token_mint,
            output_token_mint,
        ) = if quote.input_mint == pool_state.token_0_mint {
            (
                TradeDirection::ZeroForOne,
                total_token_0_amount,
                total_token_1_amount,
                token_0_mint_info,
                token_1_mint_info,
            )
        } else {
            (
                TradeDirection::OneForZero,
                total_token_1_amount,
                total_token_0_amount,
                token_1_mint_info,
                token_0_mint_info,
            )
        };

        let current_unix_timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let actual_amount_specified = get_amount_after_transfer_fee(
            quote.amount,
            if base_in {
                &input_token_mint
            } else {
                &output_token_mint
            },
            base_in,
            epoch,
        );

        let swap_result = if base_in {
            CurveCalculator::swap_base_input(
                u128::from(actual_amount_specified),
                u128::from(total_input_token_amount),
                u128::from(total_output_token_amount),
                &amm_config,
                &pool_state,
                current_unix_timestamp,
                &observation_state,
                false
            )
        } else {
            CurveCalculator::swap_base_output(
                u128::from(actual_amount_specified),
                u128::from(total_input_token_amount),
                u128::from(total_output_token_amount),
                &amm_config,
                &pool_state,
                current_unix_timestamp,
                &observation_state,
                false
            )
        }?;

        let other_amount = u64::try_from(if base_in {
            swap_result.destination_amount_swapped
        } else {
            swap_result.source_amount_swapped
        })
        .unwrap();

        let other_amount = get_amount_after_transfer_fee(
            other_amount,
            if base_in {
                &output_token_mint
            } else {
                &input_token_mint
            },
            base_in,
            epoch,
        );

        let other_amount_threshold =
            amount_with_slippage(other_amount, quote.slippage_bps as f64 / 10_000.0, !base_in);

        let (in_amount, out_amount) = if base_in {
            (quote.amount, other_amount)
        } else {
            (other_amount, quote.amount)
        };

        let fee_amount = u64::try_from(swap_result.dynamic_fee).unwrap();
        let initial_price = Decimal::from_u64(total_input_token_amount - fee_amount)
            .unwrap()
            .div(Decimal::from_u64(total_output_token_amount).unwrap());
        let final_price = Decimal::from_u128(swap_result.new_swap_source_amount)
            .unwrap()
            .div(Decimal::from_u128(swap_result.new_swap_destination_amount).unwrap());
        let price_impact =
            (Decimal::from(1).sub(initial_price.div(final_price))).mul(Decimal::from(100));

        let response = QuoteResponse {
            input_mint: quote.input_mint,
            output_mint: quote.output_mint,
            in_amount,
            out_amount,
            other_amount_threshold,
            swap_mode,
            slippage_bps: quote.slippage_bps,
            platform_fee: None, // todo!(),
            price_impact_pct: price_impact.to_string(),
            route_plan: vec![RoutePlanStep {
                swap_info: SwapInfo {
                    amm_key: self.gamma_program_id,
                    label: "Gamma".to_string(),
                    input_mint: quote.input_mint,
                    output_mint: quote.output_mint,
                    in_amount,
                    out_amount,
                    fee_amount,
                    fee_mint: quote.input_mint,
                },
                percent: 100,
            }],
            context_slot: min_context_slot,
            time_taken: start.elapsed().as_secs_f64(),
        };

        Ok(response)
    }
}

pub fn amount_with_slippage(amount: u64, slippage: f64, round_up: bool) -> u64 {
    if round_up {
        (amount as f64).mul(1_f64 + slippage).ceil() as u64
    } else {
        (amount as f64).mul(1_f64 - slippage).floor() as u64
    }
}

pub fn get_amount_after_transfer_fee<S: BaseState>(
    amount: u64,
    mint: &StateWithExtensionsMut<'_, S>,
    base_in: bool,
    epoch: u64,
) -> u64 {
    let fee = if base_in {
        get_transfer_fee(mint, epoch, amount)
    } else {
        get_transfer_inverse_fee(mint, epoch, amount)
    };

    if base_in {
        // If amount-specified is input then the protocol only gives us enough output for `input - fees``
        amount.saturating_sub(fee)
    } else {
        // If amount-specified is output then we need to provide enough input for `output + fees`
        amount.checked_add(fee).unwrap_or(0)
    }
}

/// Calculate the fee for output amount
pub fn get_transfer_inverse_fee<S: BaseState>(
    account_state: &StateWithExtensionsMut<'_, S>,
    epoch: u64,
    post_fee_amount: u64,
) -> u64 {
    let fee = if let Ok(transfer_fee_config) = account_state.get_extension::<TransferFeeConfig>() {
        let transfer_fee = transfer_fee_config.get_epoch_fee(epoch);
        if u16::from(transfer_fee.transfer_fee_basis_points) == MAX_FEE_BASIS_POINTS {
            u64::from(transfer_fee.maximum_fee)
        } else {
            transfer_fee_config
                .calculate_inverse_epoch_fee(epoch, post_fee_amount)
                .unwrap()
        }
    } else {
        0
    };
    fee
}

/// Calculate the fee for input amount
pub fn get_transfer_fee<S: BaseState>(
    account_state: &StateWithExtensionsMut<'_, S>,
    epoch: u64,
    pre_fee_amount: u64,
) -> u64 {
    let fee = if let Ok(transfer_fee_config) = account_state.get_extension::<TransferFeeConfig>() {
        transfer_fee_config
            .calculate_epoch_fee(epoch, pre_fee_amount)
            .unwrap()
    } else {
        0
    };
    fee
}
