#![allow(clippy::type_complexity)]

use accounts::AccountsGetter;
use axum::{
    routing::{get, post},
    Router,
};
use blockhash_polling::{get_blockhash_data_with_retry, start_blockhash_polling_task};
use clap::Parser;
use gfx_swap::GfxSwapClient;
use priofee::start_priofees_task;
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
mod gfx_swap;
mod handlers;
mod priofee;
mod tx_utils;
mod utils;

#[derive(Debug, Parser)]
#[clap(version, about, long_about = None)]
pub struct Opts {
    #[clap(long, env, help = "Solana cluster RPC-URL")]
    rpc_url: String,

    #[clap(long, env, help = "Protocol config address")]
    amm_config: Pubkey,

    #[clap(long, env, help = "The Gamma Program ID")]
    amm_program_id: Pubkey,

    #[clap(long, env, help = "Server host")]
    host: String,

    #[clap(long, env, help = "Server port")]
    port: String,

    #[clap(
        long,
        env,
        help = "How frequently to poll for a new blockhash(in milliseconds)"
    )]
    blockhash_poll_frequency_ms: Option<u64>,

    #[clap(long, env, help = "The URL to make priority fee requests to")]
    priofee_url: Option<String>,

    #[clap(
        long,
        env,
        help = "How many blocks to consider for priority-fee averages"
    )]
    priofee_n_blocks: Option<u16>,

    #[clap(long, env, help = "How frequently to update the priority fee response")]
    priofee_poll_frequency_secs: Option<u64>,

    #[clap(long, env, help = "The referral account, if that feature is enabled")]
    referral_account: Option<Pubkey>,

    #[clap(long, env, help = "The referral program")]
    referral_program: Option<Pubkey>,

    #[clap(subcommand)]
    mode: Mode,
}

#[derive(Debug, Parser)]
enum Mode {
    UseGrpc {
        #[clap(long, env = "GRPC_ADDR")]
        addr: String,
        #[clap(long, env = "GRPC_X_TOKEN")]
        x_token: Option<String>,
    },
    UseRpc {
        #[clap(long, env = "RPC_NEW_POOLS_FREQUENCY_SECS")]
        gpa_poll_frequency_seconds: u64,
        #[clap(long, env = "RPC_ACCOUNT_REFRESH_FREQUENCY_SECS")]
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
        opts.blockhash_poll_frequency_ms.map(Duration::from_millis),
    );
    tasks.push(blockhash_task);

    let priofees_handle = match opts.priofee_url {
        Some(url) => {
            let (handle, task) = start_priofees_task(
                url,
                opts.priofee_n_blocks,
                Some(opts.amm_program_id.to_string()),
                opts.priofee_poll_frequency_secs.map(Duration::from_secs),
            )
            .await?;
            tasks.push(task);
            Some(handle)
        }
        None => None,
    };

    let store = Arc::new(accounts::MemStore::default());
    let (amm_pools, amm_pools_task, accounts_store, accounts_updater_task) = match opts.mode {
        Mode::UseGrpc { addr, x_token } => {
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
        Mode::UseRpc {
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
        Arc::clone(&rpc_client),
        tokio_stream::wrappers::ReceiverStream::new(amm_pools),
        accounts_store,
        opts.amm_program_id,
        opts.amm_config,
    )
    .await?;
    tasks.push(account_service_task);

    let gfx_swap = GfxSwapClient {
        solana_rpc: Arc::clone(&rpc_client),
        accounts_service,
        gamma_config: opts.amm_config,
        gamma_program_id: opts.amm_program_id,
        blockhash,
        priofees_handle,
        referral: opts.referral_account,
        referral_program: opts.referral_program,
    };
    let socket_addr = format!("{}:{}", opts.host, opts.port).parse::<SocketAddr>()?;

    let app = Router::new()
        .route("/quote", get(handlers::quote::quote))
        .route("/swap", post(handlers::swap::swap_transaction))
        .route(
            "/swap-instructions",
            post(handlers::swap::swap_instructions),
        )
        .with_state(gfx_swap)
        .layer(CorsLayer::permissive());

    log::info!("Gamma Swap API running on {}", socket_addr);
    axum::Server::bind(&socket_addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}
