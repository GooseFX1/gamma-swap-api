#![allow(clippy::type_complexity)]

use accounts::AccountsGetter;
use axum::{
    routing::{get, post},
    Router,
};
use blockhash_polling::{get_blockhash_data_with_retry, start_blockhash_polling_task};
use clap::Parser;
use gfx_swap::GfxSwapClient;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

mod accounts;
mod blockhash_polling;
mod gfx_amm_api;
mod gfx_swap;
mod handlers;
mod utils;

#[derive(Debug, Parser)]
#[clap(version, about, long_about = None)]
pub struct Opts {
    #[clap(long, env = "RPC_URL")]
    rpc_url: String,
    #[clap(long, env = "AMM_CONFIG")]
    amm_config: Pubkey,
    #[clap(long, env = "AMM_PROGRAM_ID")]
    amm_program_id: Pubkey,
    #[clap(long, env = "HOST")]
    host: String,
    #[clap(long, env = "PORT")]
    port: String,
    #[clap(short, long, env = "BLOCKHASH_POLL_FREQUENCY")]
    blockhash_poll_frequency_s: u64,
    #[clap(subcommand)]
    mode: Mode,
}

#[derive(Debug, Parser)]
enum Mode {
    Grpc {
        #[clap(long, env = "GRPC_ADDR")]
        addr: String,
        #[clap(long, env = "GRPC_X_TOKEN")]
        x_token: Option<String>,
    },
    Rpc {
        #[clap(long, env = "RPC_NEW_POOLS_FREQUENCY")]
        gpa_poll_frequency_seconds: u64,
        #[clap(long, env = "RPC_ACCOUNT_REFRESH_FREQUENCY")]
        refresh_frequency_seconds: u64,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv()?;
    env_logger::init();
    let opts = Opts::parse();

    let rpc_client = Arc::new(RpcClient::new(opts.rpc_url));
    let commitment_config = CommitmentConfig::confirmed();
    let blockhash = Arc::new(RwLock::new(
        get_blockhash_data_with_retry(&rpc_client, commitment_config, 3).await?,
    ));
    let mut tasks = vec![];
    let blockhash_task = start_blockhash_polling_task(
        Arc::clone(&rpc_client),
        Arc::clone(&blockhash),
        commitment_config,
    );
    tasks.push(blockhash_task);

    let store = Arc::new(accounts::MemStore::default());
    let (amm_pools, amm_pools_task, accounts_store, accounts_updater_task) = match opts.mode {
        Mode::Grpc { addr, x_token } => {
            let (pools_task, pool_receiver) = accounts::grpc::stream::grpc_amm_pools_task(
                addr.clone(),
                x_token.clone(),
                opts.amm_program_id,
                opts.amm_config,
            );
            let (grpc_accounts, accounts_updater_task) =
                accounts::grpc::stream::grpc_accounts_updater_task(
                    addr.clone(),
                    x_token.clone(),
                    store,
                );
            (
                pool_receiver,
                pools_task,
                Arc::new(grpc_accounts) as Arc<dyn AccountsGetter>,
                accounts_updater_task,
            )
        }
        Mode::Rpc {
            gpa_poll_frequency_seconds,
            refresh_frequency_seconds,
        } => {
            let (pools_task, pool_receiver) = accounts::rpc::stream::rpc_amm_pools_task(
                Arc::clone(&rpc_client),
                opts.amm_program_id,
                opts.amm_config,
                Duration::from_secs(gpa_poll_frequency_seconds),
            );
            let (rpc_accounts, accounts_updater_task) =
                accounts::rpc::stream::rpc_accounts_updater_task(
                    Arc::clone(&rpc_client),
                    store,
                    Duration::from_secs(refresh_frequency_seconds),
                );
            (
                pool_receiver,
                pools_task,
                Arc::new(rpc_accounts) as Arc<dyn AccountsGetter>,
                accounts_updater_task,
            )
        }
    };
    tasks.extend([amm_pools_task, accounts_updater_task]);

    let (account_service_task, accounts_service) = accounts::service::bootstrap_accounts_service(
        tokio_stream::wrappers::UnboundedReceiverStream::new(amm_pools),
        accounts_store,
        Arc::clone(&rpc_client),
        opts.amm_program_id,
        opts.amm_config,
    )
    .await?;
    tasks.push(account_service_task);

    let gfx_swap = Arc::new(GfxSwapClient {
        solana_rpc: Arc::clone(&rpc_client),
        accounts_service,
        gamma_config: opts.amm_config,
        gamma_program_id: opts.amm_program_id,
        blockhash,
    });
    let socket_addr = format!("{}:{}", opts.host, opts.port).parse::<SocketAddr>()?;

    let app = Router::new()
        .route("/quote", get(handlers::quote::quote))
        .route("/swap", post(handlers::swap::swap_transaction))
        .route(
            "/swap_instructions",
            post(handlers::swap::swap_instructions),
        )
        .with_state(gfx_swap)
        .layer(CorsLayer::permissive());

    println!("Gamma Swap API running on {}", socket_addr);
    axum::Server::bind(&socket_addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}
