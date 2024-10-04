use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use log::warn;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::hash::Hash;
use tokio::sync::RwLock;

const DEFAULT_REFRESH_RATE: Duration = Duration::from_secs(5);

pub struct RecentBlockhash {
    pub hash: Hash,
    pub last_valid_block_height: u64,
}

pub async fn get_blockhash_data_with_retry(
    rpc_client: &RpcClient,
    commitment: CommitmentConfig,
    retries: u8,
) -> anyhow::Result<RecentBlockhash> {
    for i in 0..retries {
        match rpc_client
            .get_latest_blockhash_with_commitment(commitment)
            .await
        {
            Ok((hash, last_valid_block_height)) => {
                let update = RecentBlockhash {
                    hash,
                    last_valid_block_height,
                };
                return Ok(update);
            }
            Err(e) => warn!("i={}. Failed to get blockhash data: {}", i, e),
        }
    }

    Err(anyhow!(
        "Failed to get blockhash data after {} retries",
        retries
    ))
}

pub fn start_blockhash_polling_task(
    rpc_client: Arc<RpcClient>,
    blockhash_notif: Arc<RwLock<RecentBlockhash>>,
    commitment_config: CommitmentConfig,
    poll_rate: Option<Duration>,
) -> tokio::task::JoinHandle<anyhow::Result<()>> {
    tokio::spawn(async move {
        loop {
            if let Ok((hash, last_valid_block_height)) = rpc_client
                .get_latest_blockhash_with_commitment(commitment_config)
                .await
            {
                *blockhash_notif.write().await = RecentBlockhash {
                    hash,
                    last_valid_block_height,
                };
            }

            tokio::time::sleep(poll_rate.unwrap_or(DEFAULT_REFRESH_RATE)).await;
        }
    })
}
