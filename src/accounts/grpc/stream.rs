use crate::accounts::{grpc, AccountData, AccountUpdate, AccountsError, AccountsGetter};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anchor_lang::Discriminator;
use anyhow::anyhow;
use async_trait::async_trait;
use dashmap::DashSet;
use futures::StreamExt;
use gamma::states::PoolState;
use log::error;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use yellowstone_grpc_proto::geyser::{
    subscribe_request_filter_accounts_filter::Filter,
    subscribe_request_filter_accounts_filter_memcmp::Data, subscribe_update::UpdateOneof,
    CommitmentLevel, SubscribeRequest, SubscribeRequestFilterAccounts,
    SubscribeRequestFilterAccountsFilter, SubscribeRequestFilterAccountsFilterMemcmp,
};

pub struct GrpcAccounts {
    keys: Arc<DashSet<Pubkey>>,
    accounts_notifier: tokio::sync::watch::Sender<Vec<String>>,
    store: Arc<dyn AccountsGetter>,
}

pub fn grpc_amm_pools_task(
    grpc_endpoint: String,
    grpc_x_token: Option<String>,
    program_id: Pubkey,
    config: Pubkey,
) -> (
    JoinHandle<Result<(), anyhow::Error>>,
    tokio::sync::mpsc::Receiver<(Pubkey, Vec<u8>)>,
) {
    log::debug!("Starting GRPC amm pools task");
    let (new_accounts_sender, new_accounts_receiver) = tokio::sync::mpsc::channel(1000);
    let task = tokio::task::spawn({
        let grpc_endpoint = grpc_endpoint.clone();
        let grpc_x_token = grpc_x_token.clone();
        async move {
            'outer: loop {
                let mut accounts_filter: HashMap<String, SubscribeRequestFilterAccounts> =
                    HashMap::new();
                accounts_filter.insert(
                    "grpc_amm_pools_subscription".to_string(),
                    SubscribeRequestFilterAccounts {
                        account: vec![],
                        owner: vec![program_id.to_string()],
                        filters: vec![
                            SubscribeRequestFilterAccountsFilter {
                                filter: Some(Filter::Memcmp(
                                    SubscribeRequestFilterAccountsFilterMemcmp {
                                        offset: 0,
                                        data: Some(Data::Bytes(PoolState::DISCRIMINATOR.to_vec())),
                                    },
                                )),
                            },
                            SubscribeRequestFilterAccountsFilter {
                                filter: Some(Filter::Memcmp(
                                    SubscribeRequestFilterAccountsFilterMemcmp {
                                        offset: 8,
                                        data: Some(Data::Bytes(config.to_bytes().to_vec())),
                                    },
                                )),
                            },
                        ],
                    },
                );

                let program_subscription = SubscribeRequest {
                    accounts: accounts_filter,
                    commitment: Some(CommitmentLevel::Confirmed.into()),
                    ..Default::default()
                };

                log::trace!("Connecting to GRPC, endpoint={}", grpc_endpoint);
                let mut client =
                    grpc::create_grpc_connection(&grpc_endpoint, &grpc_x_token).await?;
                log::trace!("Connection complete, sending subscribe request");
                let mut account_stream = client.subscribe_once(program_subscription).await.unwrap();
                log::trace!("Sent subscribe-request successfully");

                while let Some(message) = account_stream.next().await {
                    let Ok(message) = message else {
                        // disconnected. retry the main loop and connect again
                        break;
                    };

                    let Some(update) = message.update_oneof else {
                        continue;
                    };

                    match update {
                        UpdateOneof::Account(update) => {
                            if let Some(account) = update.account {
                                let pubkey =
                                    Pubkey::new_from_array(account.pubkey[0..32].try_into()?);
                                log::trace!(
                                    "GRPC program subscription: Got new account {}",
                                    pubkey
                                );
                                if new_accounts_sender
                                    .send((pubkey, account.data))
                                    .await
                                    .is_err()
                                {
                                    error!("Receiver end of GRPC amm pools channel closed. Exiting task");
                                    break 'outer;
                                }
                            }
                        }
                        UpdateOneof::Ping(_) => {
                            log::trace!("Received ping from GRPC accounts stream");
                        }
                        _ => {
                            log::error!("Received unexpected message from GRPC accounts stream");
                        }
                    }
                }
                log::error!("Grpc stream disconnected. Reconnecting..");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            Ok(())
        }
    });

    (task, new_accounts_receiver)
}

pub fn grpc_accounts_updater_task(
    grpc_endpoint: String,
    grpc_x_token: Option<String>,
    store: Arc<dyn AccountsGetter>,
) -> (GrpcAccounts, JoinHandle<anyhow::Result<()>>) {
    let (accounts_notifier, mut accounts_watch) = tokio::sync::watch::channel(vec![]);

    log::debug!("Starting GRPC account-updater task");
    let main_task = tokio::task::spawn({
        let store = Arc::clone(&store);
        async move {
            match accounts_watch.changed().await {
                Ok(_) => {}
                Err(e) => {
                    log::error!("GRPC account streaming task failed: {} ", e);
                    return Err(anyhow!(""));
                }
            }
            let accounts = accounts_watch.borrow_and_update().clone();

            let has_started = Arc::new(tokio::sync::Notify::new());
            let mut current_task = grpc_accounts_updater_task_inner(
                grpc_endpoint.clone(),
                grpc_x_token.clone(),
                Arc::clone(&store),
                accounts,
                Arc::clone(&has_started),
            );

            while accounts_watch.changed().await.is_ok() {
                // Introduce delays to prevent creating new tasks spuriously
                tokio::time::sleep(Duration::from_secs(1)).await;
                let accounts = accounts_watch.borrow_and_update().clone();
                let has_started = Arc::new(tokio::sync::Notify::new());

                let new_task = grpc_accounts_updater_task_inner(
                    grpc_endpoint.clone(),
                    grpc_x_token.clone(),
                    Arc::clone(&store),
                    accounts,
                    Arc::clone(&has_started),
                );

                if tokio::time::timeout(Duration::from_secs(60), has_started.notified())
                    .await
                    .is_err()
                {
                    error!("Updated accounts-watching task failed to start");
                    new_task.abort();
                    continue;
                }

                log::trace!("Resetting accounts updater GRPC task");
                current_task.abort();
                current_task = new_task;
            }

            Ok(())
        }
    });

    let grpc_accounts = GrpcAccounts {
        keys: Arc::new(DashSet::new()),
        accounts_notifier,
        store,
    };

    (grpc_accounts, main_task)
}

fn grpc_accounts_updater_task_inner(
    grpc_endpoint: String,
    grpc_x_token: Option<String>,
    store: Arc<dyn AccountsGetter>,
    accounts: Vec<String>,
    has_started: Arc<Notify>,
) -> JoinHandle<anyhow::Result<()>> {
    tokio::task::spawn({
        async move {
            loop {
                if accounts.is_empty() {
                    return Ok(());
                }

                let mut connection =
                    grpc::create_grpc_connection(&grpc_endpoint, &grpc_x_token).await?;
                let mut stream = connection
                    .subscribe_once(SubscribeRequest {
                        accounts: [(
                            "grpc_accounts_update_subscription".to_string(),
                            SubscribeRequestFilterAccounts {
                                account: accounts.clone(),
                                ..Default::default()
                            },
                        )]
                        .into(),
                        commitment: Some(CommitmentLevel::Confirmed.into()),
                        ..Default::default()
                    })
                    .await?;

                while let Some(message) = stream.next().await {
                    let Ok(message) = message else {
                        // disconnected. retry the main loop and connect again
                        break;
                    };

                    let Some(update) = message.update_oneof else {
                        continue;
                    };

                    has_started.notify_one();

                    match update {
                        UpdateOneof::Account(update) => {
                            if let Some(account) = update.account {
                                let pubkey =
                                    Pubkey::new_from_array(account.pubkey[0..32].try_into()?);
                                log::trace!(
                                    "GRPC account-updater: Got account update for {}",
                                    pubkey
                                );

                                let _ = store
                                    .add_or_update_account(AccountUpdate {
                                        pubkey,
                                        data: account.data,
                                    })
                                    .await;
                            }
                        }
                        UpdateOneof::Ping(_) => {
                            log::trace!("Received ping from GRPC accounts stream");
                        }
                        _ => {
                            log::error!("Received unexpected message from GRPC accounts stream");
                        }
                    }
                }
            }
        }
    })
}

#[async_trait]
impl AccountsGetter for GrpcAccounts {
    async fn add_or_update_account(&self, update: AccountUpdate) {
        if self.keys.insert(update.pubkey) {
            let updated_accounts = self
                .keys
                .iter()
                .map(|p| p.key().to_string())
                .collect::<Vec<_>>();
            if let Err(e) = self.accounts_notifier.send(updated_accounts) {
                error!("Error updating accounts for Grpc streaming task: {}", e);
            }
            self.store.add_or_update_account(update).await;
        }
    }

    async fn get_account(&self, key: &Pubkey) -> Result<AccountData, AccountsError> {
        self.store.get_account(key).await
    }
}
