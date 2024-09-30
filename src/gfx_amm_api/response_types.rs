use jupiter_swap_api_client::serde_helpers::field_as_string;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

#[derive(Deserialize, Serialize, Debug)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(flatten)]
    pub data: T,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ApiError {
    pub success: bool,
    pub message: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ApiError {}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PoolType {
    Primary,
    Hyper,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(with = "field_as_string")]
    pub id: Pubkey,
    pub index: u16,
    pub trade_fee_rate: u64,
    pub protocol_fee_rate: u64,
    pub fund_fee_rate: u64,
    pub create_pool_fee: u64,
    #[serde(with = "field_as_string")]
    pub protocol_owner: Pubkey,
    #[serde(with = "field_as_string")]
    pub fund_owner: Pubkey,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PoolKeys {
    pub config: Config,
    #[serde(with = "field_as_string")]
    pub id: Pubkey,
    #[serde(with = "field_as_string")]
    pub program_id: Pubkey,
    #[serde(with = "field_as_string")]
    pub mint_a: Pubkey,
    #[serde(with = "field_as_string")]
    pub mint_b: Pubkey,
    pub open_time: u64,
    #[serde(with = "field_as_string")]
    pub mint_a_vault: Pubkey,
    #[serde(with = "field_as_string")]
    pub mint_b_vault: Pubkey,
    #[serde(with = "field_as_string")]
    pub authority: Pubkey,
    pub pool_type: PoolType,
    #[serde(with = "field_as_string")]
    pub mint_a_program: Pubkey,
    #[serde(with = "field_as_string")]
    pub mint_b_program: Pubkey,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PoolInfo {
    pub config: Config,
    #[serde(with = "field_as_string")]
    pub id: Pubkey,
    #[serde(with = "field_as_string")]
    pub program_id: Pubkey,
    pub open_time: u64,
    #[serde(with = "field_as_string")]
    pub mint_a_vault: Pubkey,
    #[serde(with = "field_as_string")]
    pub mint_b_vault: Pubkey,
    #[serde(with = "field_as_string")]
    pub authority: Pubkey,
    pub pool_type: PoolType,
    pub price: Option<f64>,
    pub tvl: Option<f64>,
    #[serde(with = "field_as_string")]
    pub pool_creator: Pubkey,
    pub mint_a: TokenInfo,
    pub mint_b: TokenInfo,
    pub stats: Stats,
}

#[derive(Deserialize, Serialize)]
pub enum Range {
    #[serde(rename = "24H")]
    Daily,
    #[serde(rename = "7D")]
    Weekly,
    #[serde(rename = "30D")]
    Monthly,
}

#[derive(Deserialize, Serialize)]
pub struct Stats {
    daily: StatsItem,
    weekly: StatsItem,
    monthly: StatsItem,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsItem {
    pub range: Range,
    pub fees_usd: f64,
    pub volume_token_a_usd: f64,
    pub volume_token_b_usd: f64,
    pub fees_apr_usd: f64,
    pub volume_apr_usd: f64,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenInfo {
    #[serde(with = "field_as_string")]
    pub address: Pubkey,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: u8,
    #[serde(rename = "logoURI")]
    pub logo_uri: Option<String>,
    pub daily_volume: Option<String>,
    pub freeze_authority: Option<String>,
    pub mint_authority: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedPoolInfo {
    pub current_page: u16,
    pub page_size: u16,
    pub total_pages: u16,
    pub total_items: u16,
    pub count: u16,
    pub pools: Vec<Option<PoolInfo>>,
}
