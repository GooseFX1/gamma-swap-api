use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PoolRequestConfig {
    pub pool_type: PoolReqType,
    pub sort_order: PoolOrder,
    pub sort_by: PoolSort,
    pub page_size: u16,
    pub page: u16,
}
impl Default for PoolRequestConfig {
    fn default() -> Self {
        PoolRequestConfig {
            pool_type: PoolReqType::default(),
            sort_order: PoolOrder::default(),
            sort_by: PoolSort::default(),
            page_size: 100,
            page: 1,
        }
    }
}
impl PoolRequestConfig {
    pub fn to_query(&self) -> Result<String, anyhow::Error> {
        Ok(serde_qs::to_string(&self)?)
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PoolReqType {
    Primary,
    Hyper,
    #[default]
    All,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PoolOrder {
    Asc,
    #[default]
    Desc,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PoolSort {
    #[default]
    Liquidity,
    Volume30d,
    Volume24h,
    Volume7d,
    Fee30d,
    Fee24h,
    Fee7d,
    Apr30d,
    Apr24h,
    Apr7d,
}
