use clap::Parser;
use jupiter_swap_api_client::{
    quote::{QuoteRequest, SwapMode},
    swap::SwapRequest,
    transaction_config::{PrioritizationFeeLamports, TransactionConfig},
    JupiterSwapApiClient,
};
use solana_client::{nonblocking::rpc_client::RpcClient, rpc_config::RpcSendTransactionConfig};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;
use solana_sdk::{
    commitment_config::CommitmentConfig, signature::EncodableKey, transaction::VersionedTransaction,
};
use gamma_swap_api::tx_utils::decode_logs::decode_transaction_logs;

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

    let client = JupiterSwapApiClient {
        base_path: "http://127.0.0.1:3000".to_string(),
        // base_path: "https://quote-api.jup.ag/v6".to_string()
    };

    let quote_response = client
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
        .await?;
    log::info!("Quote response: {:#?}", quote_response);

    let swap_request = SwapRequest {
        user_public_key: keypair.pubkey(),
        quote_response,
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

    let response = client.swap(&swap_request).await?;
    let tx = bincode::deserialize::<VersionedTransaction>(&response.swap_transaction)?;
    let tx = VersionedTransaction::try_new(tx.message, &[&keypair])?;

    let signature = rpc_client
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
        .await?;
    println!("View confirmed txn at: https://explorer.solana.com/tx/{}", signature);

    // Decode transaction logs
    decode_transaction_logs(&rpc_client, &signature).await?;

    Ok(())
}
