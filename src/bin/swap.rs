use jupiter_swap_api_client::{
    quote::{QuoteRequest, SwapMode},
    swap::SwapRequest,
    transaction_config::{ComputeUnitPriceMicroLamports, TransactionConfig},
    JupiterSwapApiClient,
};
use solana_client::{nonblocking::rpc_client::RpcClient, rpc_config::RpcSendTransactionConfig};
use solana_sdk::signer::Signer;
use solana_sdk::{
    commitment_config::CommitmentConfig, signature::EncodableKey, transaction::VersionedTransaction,
};
use solana_sdk::{pubkey, pubkey::Pubkey};

const MINT_1: Pubkey = pubkey!("So11111111111111111111111111111111111111112");
const MINT_2: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    dotenv::dotenv()?;
    env_logger::init();

    let host = std::env::var("HOST")?;
    let port = std::env::var("PORT")?;
    let keypair =
        solana_sdk::signature::Keypair::read_from_file("keypair.json").expect("No keypair file");
    let base_path = format!("http://{}:{}", host, port);
    let rpc_client = RpcClient::new(std::env::var("RPC_URL")?);
    log::info!("Base path: {}", base_path);

    let client = JupiterSwapApiClient {
        base_path: "http://127.0.0.1:3000".to_string(),
    };

    let quote_response = client
        .quote(&QuoteRequest {
            input_mint: MINT_1,
            output_mint: MINT_2,
            amount: 10_000_000, // 0.01 SOL
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
            compute_unit_price_micro_lamports: Some(ComputeUnitPriceMicroLamports::MicroLamports(
                100_000,
            )),
            prioritization_fee_lamports: None,
            dynamic_compute_unit_limit: true,
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
    println!("View confirmed txn at: https://solscan.io/tx/{}", signature);

    Ok(())
}
