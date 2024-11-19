use solana_client::{nonblocking::rpc_client::RpcClient, rpc_config::RpcTransactionConfig};
use solana_sdk::{commitment_config::CommitmentConfig, signature::Signature};
use solana_transaction_status::UiTransactionEncoding;
use gamma::states::SwapEvent;
use crate::tx_utils::events_instructions_parse::{parse_program_event, parse_program_instruction};

pub async fn decode_transaction_logs(rpc_client: &RpcClient, signature: &Signature, swap_event_emitted: &mut SwapEvent) -> anyhow::Result<()> {
    let tx = rpc_client.get_transaction_with_config(
        signature,
        RpcTransactionConfig {
            encoding: Some(UiTransactionEncoding::Json),
            commitment: Some(CommitmentConfig::confirmed()),
            max_supported_transaction_version: Some(0),
        },
    ).await?;
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
    // decode logs
    parse_program_event(gamma::id().to_string().as_str(), meta.clone(), swap_event_emitted)?;

    Ok(())
} 