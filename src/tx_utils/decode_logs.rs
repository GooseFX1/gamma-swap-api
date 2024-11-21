use crate::tx_utils::events_instructions_parse::{parse_program_event, parse_program_instruction};
use gamma::states::SwapEvent;
use solana_client::{nonblocking::rpc_client::RpcClient, rpc_config::RpcTransactionConfig};
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Signature};
use solana_transaction_status::UiTransactionEncoding;

pub async fn decode_transaction_logs(
    rpc_client: &RpcClient,
    signature: &Signature,
) -> anyhow::Result<SwapEvent> {
    let tx = rpc_client
        .get_transaction_with_config(
            signature,
            RpcTransactionConfig {
                encoding: Some(UiTransactionEncoding::Json),
                commitment: Some(CommitmentConfig::confirmed()),
                max_supported_transaction_version: Some(0),
            },
        )
        .await?;
    let transaction = tx.transaction;
    // get meta
    let meta = if transaction.meta.is_some() {
        transaction.meta
    } else {
        None
    };
    // get encoded_transaction
    let encoded_transaction = transaction.transaction;
    // decode instruction data
    parse_program_instruction(
        gamma::id().to_string().as_str(),
        encoded_transaction,
        meta.clone(),
    )?;
    let mut swap_event: SwapEvent = SwapEvent {
        pool_id: Pubkey::default(),
        input_vault_before: 0,
        output_vault_before: 0,
        input_amount: 0,
        output_amount: 0,
        input_transfer_fee: 0,
        output_transfer_fee: 0,
        base_input: false,
        dynamic_fee: 0,
    };
    // decode logs
    parse_program_event(
        gamma::id().to_string().as_str(),
        meta.clone(),
        &mut swap_event,
    )?;

    Ok(swap_event)
}
