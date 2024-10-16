use clap::Parser;
use jupiter_swap_api_client::{
    quote::{QuoteRequest, SwapMode},
    JupiterSwapApiClient,
};
use solana_sdk::pubkey::Pubkey;

#[derive(Parser)]
pub struct Config {
    #[clap(long, env = "HOST")]
    server_host: String,
    #[clap(long, env = "PORT")]
    server_port: String,
    #[clap(long, env)]
    input_mint: Pubkey,
    #[clap(long, env)]
    output_mint: Pubkey,
    #[clap(long, env)]
    amount: u64,
}

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    dotenv::dotenv()?;
    env_logger::init();

    let opts = Config::parse();
    let base_path = format!("http://{}:{}", opts.server_host, opts.server_port);
    log::info!("Base path: {}", base_path);

    let quote_request = QuoteRequest {
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
