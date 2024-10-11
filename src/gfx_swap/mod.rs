pub mod quote;
pub mod swap;

use crate::accounts::service::AccountsService;
use crate::blockhash_polling::RecentBlockhash;
use crate::priofee::PrioFeesHandle;
use std::sync::Arc;

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct GfxSwapClient {
    /// Solana RPC client
    pub solana_rpc: Arc<RpcClient>,
    /// Handle for retrieving accountInfos
    pub accounts_service: AccountsService,
    /// The Gamma protocol config address
    pub gamma_config: Pubkey,
    /// The Gamma program
    pub gamma_program_id: Pubkey,
    /// Handle for getting latest blockhash
    pub blockhash: Arc<RwLock<RecentBlockhash>>,
    /// Handle for getting latest priority fees
    pub priofees_handle: Option<PrioFeesHandle>,
    /// The referral account
    pub referral: Option<Pubkey>,
    /// The referral program
    pub referral_program: Option<Pubkey>,
}
