use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail};
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::RwLock;

// https://marketplace.quicknode.com/add-on/solana-priority-fee

const DEFAULT_N_BLOCKS: u16 = 100;
const DEFAULT_DURATION: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub struct PrioFeesHandle {
    latest: Arc<RwLock<QnPriofee>>,
}

impl PrioFeesHandle {
    pub async fn get_latest_priofee(&self) -> QnPriofee {
        *self.latest.read().await
    }
}

pub async fn start_priofees_task(
    url: String,
    n_blocks: Option<u16>,
    account: Option<String>,
    poll_duration: Option<Duration>,
) -> anyhow::Result<(PrioFeesHandle, tokio::task::JoinHandle<anyhow::Result<()>>)> {
    let response = qn_priority_fee_request(&url, n_blocks, account.clone()).await?;
    let latest = Arc::new(RwLock::new(response));
    let mut interval = tokio::time::interval(poll_duration.unwrap_or(DEFAULT_DURATION));
    let task = tokio::spawn({
        let latest = Arc::clone(&latest);
        let url = url.clone();
        let account = account.clone();
        async move {
            loop {
                interval.tick().await;
                match qn_priority_fee_request(&url, n_blocks, account.clone()).await {
                    Ok(response) => {
                        // log::debug!("{response:#?}");
                        *latest.write().await = response;
                    }
                    Err(e) => error!("{}", e),
                }
            }
        }
    });
    let handle = PrioFeesHandle { latest };
    Ok((handle, task))
}

pub async fn qn_priority_fee_request(
    url: &str,
    n_blocks: Option<u16>,
    account: Option<String>,
) -> anyhow::Result<QnPriofee> {
    let last_n_blocks = n_blocks.unwrap_or(DEFAULT_N_BLOCKS);
    let mut params = json!({
        "last_n_blocks": last_n_blocks
    });
    if let Some(account) = account {
        params
            .as_object_mut()
            .map(|m| m.insert("account".to_string(), json!(account)));
    }
    let response = reqwest::Client::new()
        .post(url)
        .header("Content-Type", "application/json")
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "qn_estimatePriorityFees",
            "params": params
        }))
        .send()
        .await;

    let mut json = match response {
        Ok(response) => response.json::<serde_json::Value>().await?,
        Err(err) => bail!("Priofee req send error: {err}"),
    };

    if let Some(result) = json.get_mut("result").map(|res| res.take()) {
        Ok(serde_json::from_value(result)?)
    } else if let Some(error) = json.get_mut("error").map(|err| err.take()) {
        Err(anyhow!("qn_estimatePriorityFees error: {}", error))
    } else {
        Err(anyhow!("qn_estimatePriorityFees error: Invalid response"))
    }
}

// #[derive(Clone, Deserialize, Debug)]
// struct Error {
//     #[allow(unused)]
//     code: i32,
//     message: String,
//     data: String,
// }

#[derive(Copy, Clone, Deserialize, Debug)]
pub struct QnPriofee {
    #[allow(unused)]
    context: Context,
    /// It provides estimates for priority fees (in microlamports) based on per-compute-unit metrics
    pub per_compute_unit: Priority,
    /// It provides estimates for priority fees (in lamports) based on per-transaction metrics
    pub per_transaction: Priority,
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
struct Context {
    pub slot: u64,
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub struct Priority {
    /// Fee estimate for 95th percentile
    pub extreme: u64,
    /// Fee estimate for 80th percentile
    pub high: u64,
    /// Fee estimate for 60th percentile
    pub medium: u64,
    /// Fee estimate for 40th percentile
    pub low: u64,
    // percentiles:
}
