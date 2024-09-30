use super::{AccountsError, AccountsGetter};
use crate::accounts::{rpc, AccountData, AccountUpdate, PoolSlice};
use crate::utils::get_keys_for_pool_exclusive;
use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use log::error;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use tokio::task::JoinHandle;

pub async fn bootstrap_accounts_service(
    mut amm_pools: impl Stream<Item = (Pubkey, Vec<u8>)> + std::marker::Unpin + Send + 'static,
    accounts_store: Arc<dyn AccountsGetter>,
    rpc_client: Arc<RpcClient>,
    program_id: Pubkey,
    config: Pubkey,
) -> anyhow::Result<(JoinHandle<Result<(), anyhow::Error>>, AccountsService)> {
    log::debug!("Bootstrapping accounts service");
    let mut processed_pools = HashSet::<Pubkey>::new();
    let mut pool_keys = rpc::get_amm_pool_pubkeys(&rpc_client, &config, &program_id).await?;
    log::debug!("Got {} pools for program and config", pool_keys.len());
    pool_keys.push(config);

    let mut pools = rpc::get_multiple_account_data(&rpc_client, &pool_keys).await?;
    let config_data = pools.pop();
    match config_data {
        Some((pubkey, data)) if pubkey == config && data.is_some() => {
            accounts_store
                .add_or_update_account(AccountUpdate {
                    pubkey,
                    data: data.unwrap(),
                })
                .await;
        }
        _ => {
            error!("Failed to get amm config from RPC");
        }
    }
    for (pool, pool_data) in pools {
        let Some(data) = pool_data else {
            error!(
                "Got null data for pool {} from rpc in account service bootstrap",
                pool
            );
            continue;
        };

        process_amm_pool(
            &mut processed_pools,
            Arc::clone(&accounts_store),
            &rpc_client,
            &program_id,
            pool,
            data,
        )
        .await;
    }

    let task = tokio::task::spawn({
        let rpc_client = Arc::clone(&rpc_client);
        let mut processed_pools = HashSet::<Pubkey>::new();
        let accounts_store = Arc::clone(&accounts_store);
        async move {
            while let Some((pool, pool_data)) = amm_pools.next().await {
                process_amm_pool(
                    &mut processed_pools,
                    Arc::clone(&accounts_store),
                    &rpc_client,
                    &program_id,
                    pool,
                    pool_data,
                )
                .await;
            }
            Ok(())
        }
    });

    let service = AccountsService { accounts_store };

    Ok((task, service))
}

async fn process_amm_pool(
    processed_pools: &mut HashSet<Pubkey>,
    accounts_store: Arc<dyn AccountsGetter>,
    rpc_client: &RpcClient,
    program_id: &Pubkey,
    pool: Pubkey,
    pool_data: Vec<u8>,
) {
    let Some(data) = PoolSlice::decode(&pool_data, None) else {
        error!("Failed to decode pool slice for amm pool {}", pool);
        return;
    };

    // Prevent duplicate processing
    if !processed_pools.contains(&pool) {
        log::trace!("Got new pool {}", pool);
        let keys = get_keys_for_pool_exclusive(&pool, &data, program_id);
        let Ok(accounts) = rpc::get_multiple_account_data(rpc_client, &keys).await else {
            error!("Failed to get fetch accounts for amm pool {}", pool);
            return;
        };
        _ = processed_pools.insert(pool);

        accounts_store
            .add_or_update_account(AccountUpdate {
                pubkey: pool,
                data: pool_data,
            })
            .await;

        for (pubkey, account) in accounts.into_iter() {
            let Some(data) = account else {
                // this should be unreachable
                error!(
                    "Got null account data from RPC. pool={}. account={}",
                    pool, pubkey
                );
                continue;
            };
            accounts_store
                .add_or_update_account(AccountUpdate { pubkey, data })
                .await;
        }
    }
}

#[derive(Clone)]
pub struct AccountsService {
    accounts_store: Arc<dyn AccountsGetter>,
}

#[async_trait]
impl AccountsGetter for AccountsService {
    async fn add_or_update_account(&self, _update: AccountUpdate) {}

    async fn get_account(&self, key: &Pubkey) -> Result<AccountData, AccountsError> {
        self.accounts_store.get_account(key).await
    }
}
