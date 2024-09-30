pub mod quote;
pub mod swap;

use crate::accounts::service::AccountsService;
use crate::blockhash_polling::RecentBlockhash;
use std::sync::Arc;

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::RwLock;

pub struct GfxSwapClient {
    pub solana_rpc: Arc<RpcClient>,
    pub accounts_service: AccountsService,
    pub gamma_config: Pubkey,
    pub gamma_program_id: Pubkey,
    pub blockhash: Arc<RwLock<RecentBlockhash>>,
}
