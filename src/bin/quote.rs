use jupiter_swap_api_client::{
    quote::{QuoteRequest, SwapMode},
    JupiterSwapApiClient,
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
    let base_path = format!("http://{}:{}", host, port);
    log::info!("Base path: {}", base_path);

    let quote_request = QuoteRequest {
        input_mint: MINT_1,
        output_mint: MINT_2,
        amount: 1_000_000_000,
        swap_mode: Some(SwapMode::ExactIn),
        slippage_bps: 1000,
        platform_fee_bps: None,
        dexes: None,
        excluded_dexes: None,
        only_direct_routes: None,
        as_legacy_transaction: None,
        max_accounts: None,
        quote_type: None,
    };

    let client = JupiterSwapApiClient {
        base_path: "http://127.0.0.1:3000".to_string(),
    };
    let quote_response = client.quote(&quote_request).await?;
    log::info!("Quote response from Gamma API: {:#?}", quote_response);

    let client = JupiterSwapApiClient {
        base_path: "https://quote-api.jup.ag/v6".to_string(),
    };
    let quote_response = client.quote(&quote_request).await?;
    log::info!("Quote response from Jupiter API: {:#?}", quote_response);

    Ok(())
}
