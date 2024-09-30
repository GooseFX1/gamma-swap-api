use jupiter_swap_api_client::{
    quote::{QuoteRequest, SwapMode},
    JupiterSwapApiClient,
};
use solana_sdk::{pubkey, pubkey::Pubkey};

// const DEVNET_MINT_1: Pubkey = pubkey!("FNBmFaQWsEpHXpTe3z4oNk8BKgcSMaZz9PDFYjgoDABo");
// const DEVNET_MINT_2: Pubkey = pubkey!("HGRjfGyepfNw7HbzPUS8jmoJ3uFzB4FB6NSRgKsr6ECA");
// const DEVNET_MINT_3: Pubkey = pubkey!("GcwnWyNfg5p3KVSHHn6xvgrYkCYKPYnRKpDn9o7BPiRJ");
// const DEVNET_MINT_4: Pubkey = pubkey!("Fc9eSn5QpAiPAmT3UFpDd6ExTeQ4MP7X8R3qcfUCFG1T");
// const DEVNET_MINT_5: Pubkey = pubkey!("CPfxKMrELo1tdtNtxXgGU8WZkc3EMYXNTwjkpP7A5PHC");
// const DEVNET_MINT_6: Pubkey = pubkey!("HWAhSw5JMWAVxZYbKpC823Dksy2fq5tQNZnFHPtpj4T4");

const MINT_1: Pubkey = pubkey!("N4CdHcZYMj7DufSu89m1gi3RFxt8NiJQ9PmfNg8kc8P");
const MINT_2: Pubkey = pubkey!("N5Y2m9HSPDBr8ft6UVWL4vLoaBfqPYawxhM1uEMx5Gk");
// config: Fc9eSn5QpAiPAmT3UFpDd6ExTeQ4MP7X8R3qcfUCFG1T
// state: AboLeuWSp48nhmstX1Psc3M66UQfTFQEecC378BrtyeS
// input-vault: 2NKXxPnk4DBvWFA524PxhusmzJd6YAtZmSiU25EkCKSy
// output-vault: 2NKXxPnk4DBvWFA524PxhusmzJd6YAtZmSiU25EkCKSy

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
            amount: 1000,
            swap_mode: Some(SwapMode::ExactIn),
            slippage_bps: 10_000,
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
