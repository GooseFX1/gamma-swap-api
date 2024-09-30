pub mod grpc;
pub mod rpc;
pub mod service;

use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use dashmap::DashMap;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use thiserror::Error;

pub type AccountData = Vec<u8>;

pub struct AccountUpdate {
    pub pubkey: Pubkey,
    pub data: AccountData,
}

#[derive(Debug, Error)]
pub enum AccountsError {
    #[error("Account not found")]
    NotFound,
    #[error(transparent)]
    Failed(#[from] anyhow::Error),
}

#[async_trait]
pub trait AccountsGetter: Send + Sync {
    async fn add_or_update_account(&self, update: AccountUpdate);

    async fn get_account(&self, key: &Pubkey) -> Result<AccountData, AccountsError>;
}

#[derive(Default)]
pub struct MemStore {
    accounts_map: Arc<DashMap<Pubkey, AccountData>>,
}

#[async_trait]
impl AccountsGetter for MemStore {
    async fn add_or_update_account(&self, update: AccountUpdate) {
        let _ = self.accounts_map.insert(update.pubkey, update.data);
    }

    async fn get_account(&self, key: &Pubkey) -> Result<AccountData, AccountsError> {
        Ok(self
            .accounts_map
            .get(key)
            .map(|v| v.clone())
            .ok_or(AccountsError::NotFound)?)
    }
}

pub struct SolanaRpcStore {
    rpc_client: Arc<RpcClient>,
}

impl SolanaRpcStore {
    #[allow(unused)]
    pub fn new(rpc_client: Arc<RpcClient>) -> Self {
        SolanaRpcStore { rpc_client }
    }
}

#[async_trait]
impl AccountsGetter for SolanaRpcStore {
    async fn add_or_update_account(&self, _update: AccountUpdate) {}

    async fn get_account(&self, key: &Pubkey) -> Result<AccountData, AccountsError> {
        Ok(self
            .rpc_client
            .get_account_data(key)
            .await
            .map_err(|e| anyhow!("{}", e))?)
    }
}

pub struct PoolSlice {
    pub token_0_mint: Pubkey,
    pub token_1_mint: Pubkey,
}

impl PoolSlice {
    pub const OFFSET: usize = 168;
    pub const LENGTH: usize = 64;

    pub fn decode(data: &[u8], offset: Option<usize>) -> Option<PoolSlice> {
        let offset = offset.unwrap_or(Self::OFFSET);
        if data.len() < Self::LENGTH + offset {
            return None;
        }

        Some(PoolSlice {
            token_0_mint: Pubkey::new_from_array(data[offset..offset + 32].try_into().ok()?),
            token_1_mint: Pubkey::new_from_array(data[offset + 32..offset + 64].try_into().ok()?),
        })
    }
}
