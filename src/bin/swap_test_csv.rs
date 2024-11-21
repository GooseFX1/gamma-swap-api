use clap::Parser;
use core::convert::From;
use csv::Writer;
use gamma_swap_api::tx_utils::decode_logs::decode_transaction_logs;
use jupiter_swap_api_client::{
    quote::{PlatformFee, QuoteRequest, SwapMode},
    swap::SwapRequest,
    transaction_config::{PrioritizationFeeLamports, TransactionConfig},
    JupiterSwapApiClient,
};
use serde::{Deserialize, Serialize};
use solana_client::{nonblocking::rpc_client::RpcClient, rpc_config::RpcSendTransactionConfig};
use solana_sdk::{
    commitment_config::CommitmentConfig, pubkey::Pubkey, signature::EncodableKey, signer::Signer,
    transaction::VersionedTransaction,
};
use tokio::time::{sleep, Duration};

#[derive(Parser)]
pub struct Config {
    /// The host of the api server to connect to
    #[clap(long, env = "HOST")]
    server_host: String,
    /// The port of the api server to connect to
    #[clap(long, env = "PORT")]
    server_port: String,
    /// The input mint for the swap
    #[clap(long, env)]
    input_mint: Pubkey,
    /// The output mint for the swap
    #[clap(long, env)]
    output_mint: Pubkey,
    /// The amount provided for the swap
    #[clap(long, env)]
    amount: u64,
}

#[derive(Serialize, Deserialize)]
struct SwapRecord {
    pool_id: String,
    input_mint: String,
    quote_in_amount: u64,
    output_mint: String,
    quote_out_amount: u64,
    other_amount_threshold: u64,
    swap_mode: String,
    slippage_bps: u16,
    platform_fee_amount: u64,
    platform_fee_bps: u8,
    price_impact_pct: String,
    time_taken: f64,
    input_vault_before: u64,
    output_vault_before: u64,
    swap_input_amount: u64,
    swap_output_amount: u64,
    swap_input_transfer_fee: u64,
    swap_output_transfer_fee: u64,
    swap_dynamic_fee: u128,
}

// Define a wrapper struct around PlatformFee
struct PlatformFeeWrapper(PlatformFee);

// Implement Default for the wrapper
impl Default for PlatformFeeWrapper {
    fn default() -> Self {
        PlatformFeeWrapper(PlatformFee {
            amount: 0,
            fee_bps: 0,
        })
    }
}

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    dotenv::dotenv()?;
    env_logger::init();

    let opts = Config::parse();
    let keypair =
        solana_sdk::signature::Keypair::read_from_file("keypair.json").expect("No keypair file");
    log::info!("pubkey: {}", keypair.pubkey());
    let base_path = format!("http://{}:{}", opts.server_host, opts.server_port);
    let rpc_client = RpcClient::new(std::env::var("RPC_URL")?);
    log::info!("Base path: {}", base_path);

    let _client = JupiterSwapApiClient {
        base_path: "http://127.0.0.1:3000".to_string(),
        // base_path: "https://quote-api.jup.ag/v6".to_string()
    };

    // Open a CSV file to write the results
    let mut wtr = Writer::from_path("swap_results.csv")?;
    // Write CSV headers
    wtr.write_record(&[
        "pool_id",
        "input_mint",
        "quote_in_amount",
        "output_mint",
        "quote_out_amount",
        "other_amount_threshold",
        "swap_mode",
        "slippage_bps",
        "platform_fee_amount",
        "platform_fee_bps",
        "price_impact_pct",
        "time_taken",
        "input_vault_before",
        "output_vault_before",
        "swap_input_amount",
        "swap_output_amount",
        "swap_input_transfer_fee",
        "swap_output_transfer_fee",
        "swap_dynamic_fee",
    ])?;

    for _ in 0..100 {
        let client = JupiterSwapApiClient {
            base_path: "http://127.0.0.1:3000".to_string(),
            // base_path: "https://quote-api.jup.ag/v6".to_string()
        };

        let quote_response = match client
            .quote(&QuoteRequest {
                input_mint: opts.input_mint,
                output_mint: opts.output_mint,
                amount: opts.amount,
                swap_mode: Some(SwapMode::ExactIn),
                slippage_bps: 1000,
                platform_fee_bps: None,
                dexes: None,
                excluded_dexes: None,
                only_direct_routes: None,
                as_legacy_transaction: None,
                max_accounts: None,
                quote_type: None,
            })
            .await
        {
            Ok(response) => response,
            Err(e) => {
                log::error!("Failed to get quote: {}", e);
                continue; // Skip to the next iteration
            }
        };
        log::info!("Quote response: {:#?}", quote_response);

        let swap_request = SwapRequest {
            user_public_key: keypair.pubkey(),
            quote_response: quote_response.clone(),
            config: TransactionConfig {
                wrap_and_unwrap_sol: true,
                fee_account: None,
                destination_token_account: None,
                compute_unit_price_micro_lamports: None,
                prioritization_fee_lamports: Some(PrioritizationFeeLamports::AutoMultiplier(10)),
                dynamic_compute_unit_limit: false,
                as_legacy_transaction: false,
                use_shared_accounts: false,
                use_token_ledger: false,
            },
        };

        let response = match client.swap(&swap_request).await {
            Ok(response) => response,
            Err(e) => {
                log::error!("Swap failed: {}", e);
                continue; // Skip to the next iteration
            }
        };

        let tx = match bincode::deserialize::<VersionedTransaction>(&response.swap_transaction) {
            Ok(tx) => tx,
            Err(e) => {
                log::error!("Failed to deserialize transaction: {}", e);
                continue; // Skip to the next iteration
            }
        };

        let tx = match VersionedTransaction::try_new(tx.message, &[&keypair]) {
            Ok(tx) => tx,
            Err(e) => {
                log::error!("Failed to create versioned transaction: {}", e);
                continue; // Skip to the next iteration
            }
        };

        let signature = match rpc_client
            .send_and_confirm_transaction_with_spinner_and_config(
                &tx,
                CommitmentConfig::confirmed(),
                RpcSendTransactionConfig {
                    skip_preflight: true,
                    preflight_commitment: Some(rpc_client.commitment().commitment),
                    max_retries: Some(0),
                    ..RpcSendTransactionConfig::default()
                },
            )
            .await
        {
            Ok(signature) => signature,
            Err(e) => {
                log::error!("Transaction failed: {}", e);
                continue; // Skip to the next iteration
            }
        };
        println!(
            "View confirmed txn at: https://explorer.solana.com/tx/{}",
            signature
        );

        // Decode transaction logs to get SwapInfo and SwapEvent
        let swap_event = match decode_transaction_logs(&rpc_client, &signature).await {
            Ok(event) => event,
            Err(e) => {
                log::error!("Failed to decode transaction logs: {}", e);
                continue; // Skip to the next iteration
            }
        };
        println!("{:#?}", swap_event);

        let platform_fee: PlatformFee =
            if let Some(platform_fee) = quote_response.clone().platform_fee {
                platform_fee
            } else {
                PlatformFeeWrapper::default().0 // Use default value if platform_fee is None
            };

        let swap_mode_string: String = match quote_response.swap_mode {
            SwapMode::ExactIn => "ExactIn".to_string(),
            SwapMode::ExactOut => "ExactOut".to_string(),
        };

        let swap_record = SwapRecord {
            pool_id: swap_event.pool_id.to_string(),
            input_mint: quote_response.clone().input_mint.to_string(),
            quote_in_amount: quote_response.clone().in_amount,
            output_mint: quote_response.clone().output_mint.to_string(),
            quote_out_amount: quote_response.clone().out_amount,
            other_amount_threshold: quote_response.clone().other_amount_threshold,
            swap_mode: swap_mode_string,
            slippage_bps: quote_response.clone().slippage_bps,
            platform_fee_amount: platform_fee.amount,
            platform_fee_bps: platform_fee.fee_bps,
            price_impact_pct: quote_response.clone().price_impact_pct,
            time_taken: quote_response.clone().time_taken,
            input_vault_before: swap_event.input_vault_before,
            output_vault_before: swap_event.output_vault_before,
            swap_input_amount: swap_event.input_amount,
            swap_output_amount: swap_event.output_amount,
            swap_input_transfer_fee: swap_event.input_transfer_fee,
            swap_output_transfer_fee: swap_event.output_transfer_fee,
            swap_dynamic_fee: swap_event.dynamic_fee,
        };
        println!("writing to csv");
        wtr.serialize(swap_record)?;

        // Sleep for a short duration to avoid rate limits
        sleep(Duration::from_secs(1)).await;
    }

    wtr.flush()?;
    Ok(())
}
