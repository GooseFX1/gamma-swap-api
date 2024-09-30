mod request_types;
mod response_types;
mod serde_helpers;

use anyhow::Context;
use request_types::PoolRequestConfig;
use response_types::{ApiError, ApiResponse, Config, PaginatedPoolInfo, PoolInfo, PoolKeys};
use solana_sdk::pubkey::Pubkey;
use thiserror::Error;

const GET_CONFIG: &str = "/v1/config";
const GET_POOLS_BY_IDS: &str = "/v1/pool/info/ids";
const GET_POOLS_BY_MINTS: &str = "/v1/pool/info/mints";
const LIST_POOLS: &str = "/v1/pool/info/all";
const GET_POOL_KEYS_BY_IDS: &str = "/v1/pool/keys/ids";

pub struct GfxApiClient {
    base_url: String,
}
impl GfxApiClient {
    pub fn new(base_url: String) -> Self {
        GfxApiClient { base_url }
    }
}

#[derive(Debug, Error)]
pub enum GfxApiError {
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    ApiError(#[from] ApiError),
    #[error("{0} not found")]
    NotFound(Pubkey),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Generic(#[from] anyhow::Error),
}

impl GfxApiClient {
    pub async fn get_config(&self, id: &Pubkey) -> Result<Config, GfxApiError> {
        let url = format!("{}{}?id={}", self.base_url, GET_CONFIG, id);
        self.make_request(url).await
    }

    pub async fn get_pool_keys(
        &self,
        ids: &[Pubkey],
    ) -> Result<Vec<Option<PoolKeys>>, GfxApiError> {
        let id_list = ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let url = format!("{}{}?ids={}", self.base_url, GET_POOL_KEYS_BY_IDS, id_list);
        self.make_request(url).await
    }

    pub async fn get_pool_info(
        &self,
        ids: &[Pubkey],
    ) -> Result<Vec<Option<PoolInfo>>, GfxApiError> {
        let id_list = ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let url = format!("{}{}?ids={}", self.base_url, GET_POOLS_BY_IDS, id_list);
        self.make_request(url).await
    }

    pub async fn get_pools_by_mints(
        &self,
        mint1: &Pubkey,
        mint2: Option<&Pubkey>,
        config: PoolRequestConfig,
    ) -> Result<Vec<PaginatedPoolInfo>, GfxApiError> {
        let mut url = format!("{}{}?mint1={}", self.base_url, GET_POOLS_BY_MINTS, mint1);
        if let Some(mint2) = mint2 {
            url = format!("{}&mint2={}", url, mint2)
        }
        url = format!("{}&{}", url, config.to_query()?);
        self.make_request(url).await
    }

    pub async fn get_pools(
        &self,
        config: PoolRequestConfig,
    ) -> Result<Vec<PaginatedPoolInfo>, GfxApiError> {
        let url = format!("{}{}&{}", self.base_url, LIST_POOLS, config.to_query()?);
        self.make_request(url).await
    }

    async fn make_request<T: serde::de::DeserializeOwned>(
        &self,
        url: String,
    ) -> Result<T, GfxApiError> {
        let json = reqwest::Client::new()
            .get(&url)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;
        let success = json
            .get("success")
            .and_then(|v| v.as_bool())
            .context("Invalid gfx-amm api response")?;
        if success {
            Ok(serde_json::from_value::<ApiResponse<T>>(json)?.data)
        } else {
            Err(serde_json::from_value::<ApiError>(json)?.into())
        }
    }
}
