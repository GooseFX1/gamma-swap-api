use jupiter_swap_api_client::{
    quote::{QuoteRequest, SwapMode},
    JupiterSwapApiClient,
};
use solana_sdk::{pubkey, pubkey::Pubkey};

// const PROGRAM_ID: Pubkey = pubkey!("gaMmp8CxTCKFbtpoGrcxccDtJJSRyF7hzTkKcuPDbRG");
// const CONFIG: Pubkey = pubkey!("FMqMyfBXw3FKjsBb3fkcpNNdfVUSUWhfHkSUt2a87Q5Z");
// const MINT_1: Pubkey = pubkey!("N6QvkdoGTkYN5f1uHH1rBzyiZhh7yE5twrp8EpzivuS");
// const MINT_2: Pubkey = pubkey!("N796TBCqdm61LNJ8GXAHJBW7uBPWAnru7bv5YS3pV4S");
// const POOL: Pubkey = pubkey!(93EnCRgiDKg6PpBZ6VsMqTUEJjD4pWRwiyEuNEERnpCV)

// const PROGRAM_ID: Pubkey = pubkey!("GAMMA7meSFWaBXF25oSUgmGRwaW6sCMFLmBNiMSdbHVT");
// const CONFIG: Pubkey = pubkey!("68yDnv1sDzU3L2cek5kNEszKFPaK9yUJaC4ghV5LAXW6");
const MINT_1: Pubkey = pubkey!("So11111111111111111111111111111111111111112");
const MINT_2: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
// const POOL: Pubkey = pubkey!(Hjm1F98vgVdN7Y9L46KLqcZZWyTKS9tj9ybYKJcXnSng);

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    dotenv::dotenv()?;
    env_logger::init();

    let host = std::env::var("HOST")?;
    let port = std::env::var("PORT")?;
    let base_path = format!("http://{}:{}", host, port);
    log::info!("Base path: {}", base_path);

    let client = JupiterSwapApiClient {
        base_path: "http://127.0.0.1:3000".to_string(),
    };

    let quote_response = client
        .quote(&QuoteRequest {
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
        })
        .await?;

    log::info!("Quote response: {:#?}", quote_response);
    Ok(())
}
