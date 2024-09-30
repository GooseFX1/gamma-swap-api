use crate::accounts::{rpc, AccountData, AccountUpdate, AccountsError, AccountsGetter};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use dashmap::DashSet;
use log::error;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use tokio::task::JoinHandle;

pub struct RpcAccounts {
    keys: Arc<DashSet<Pubkey>>,
    store: Arc<dyn AccountsGetter>,
}

pub fn rpc_amm_pools_task(
    rpc_client: Arc<RpcClient>,
    program_id: Pubkey,
    config: Pubkey,
    poll_frequency: Duration,
) -> (
    JoinHandle<Result<(), anyhow::Error>>,
    tokio::sync::mpsc::UnboundedReceiver<(Pubkey, Vec<u8>)>,
) {
    let (new_accounts_sender, new_accounts_receiver) = tokio::sync::mpsc::unbounded_channel();

    log::debug!("Starting RPC new pools task");
    let task = tokio::task::spawn(async move {
        let mut interval = tokio::time::interval(poll_frequency);

        loop {
            interval.tick().await;

            let Ok(keys) = rpc::get_amm_pool_pubkeys(&rpc_client, &config, &program_id).await
            else {
                error!("Failed getting amm pool keys by GPA");
                continue;
            };
            log::debug!("Got {} pools for program and config", keys.len());

            let Ok(pools) = rpc::get_multiple_account_data(&rpc_client, &keys).await else {
                error!("Failed getting multiple accountInfo by RPC for amm-pools-task");
                continue;
            };

            for (pool, account) in pools {
                if let Some(account) = account {
                    if new_accounts_sender.send((pool, account)).is_ok() {
                        log::trace!("Sent pool {} to account service task", pool);
                    } else {
                        log::error!("Failed to send pool {} to account service task", pool);
                    }
                } else {
                    error!("Got null data for pool {} from rpc", pool);
                }
            }
        }
    });
    (task, new_accounts_receiver)
}

pub fn rpc_accounts_updater_task(
    rpc_client: Arc<RpcClient>,
    store: Arc<dyn AccountsGetter>,
    refresh_frequency: Duration,
) -> (RpcAccounts, JoinHandle<anyhow::Result<()>>) {
    let keys = Arc::new(DashSet::<Pubkey>::new());
    log::debug!("Starting RPC account-updater task");

    let task = tokio::task::spawn({
        let mut refresh_interval = tokio::time::interval(refresh_frequency);
        let rpc_client = Arc::clone(&rpc_client);
        let store = Arc::clone(&store);
        let keys = Arc::clone(&keys);

        async move {
            loop {
                let keys_to_refresh = keys.iter().map(|v| *v.key()).collect::<Vec<_>>();
                let accounts =
                    rpc::get_multiple_account_data(&rpc_client, &keys_to_refresh).await?;

                for (pubkey, account) in accounts.into_iter() {
                    if let Some(data) = account {
                        let _ = store
                            .add_or_update_account(AccountUpdate { pubkey, data })
                            .await;
                    }
                }
                _ = refresh_interval.tick().await;
            }
        }
    });
    let rpc_accounts = RpcAccounts { keys, store };
    (rpc_accounts, task)
}

#[async_trait]
impl AccountsGetter for RpcAccounts {
    async fn add_or_update_account(&self, update: AccountUpdate) {
        self.keys.insert(update.pubkey);
        self.store.add_or_update_account(update).await;
    }

    async fn get_account(&self, key: &Pubkey) -> Result<AccountData, AccountsError> {
        self.store.get_account(key).await
    }
}
