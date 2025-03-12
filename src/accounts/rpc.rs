pub mod stream;

use super::AccountData;

use anchor_lang::Discriminator;
use futures::stream::FuturesOrdered;
use futures::StreamExt;
use gamma::states::PoolState;
use solana_account_decoder::UiDataSliceConfig;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_client::rpc_filter::{Memcmp, RpcFilterType};
use solana_sdk::account::Account;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;

pub async fn get_amm_pool_pubkeys(
    rpc_client: &RpcClient,
    config: &Pubkey,
    program_id: &Pubkey,
) -> anyhow::Result<Vec<Pubkey>> {
    let accounts = get_pool_program_accounts(
        rpc_client,
        config,
        program_id,
        Some(UiDataSliceConfig {
            offset: 0,
            length: 0,
        }),
    )
    .await?;

    Ok(accounts.into_iter().map(|(key, _)| key).collect())
}

pub async fn get_multiple_account_data(
    rpc_client: &RpcClient,
    keys: &[Pubkey],
) -> anyhow::Result<Vec<(Pubkey, Option<AccountData>)>> {
    let mut tasks = FuturesOrdered::new();
    let mut accounts_vec = Vec::with_capacity(keys.len());
    for chunk in keys.chunks(100) {
        tasks.push_back(async {
            let response = rpc_client
                .get_multiple_accounts_with_config(
                    chunk,
                    RpcAccountInfoConfig {
                        encoding: Some(solana_account_decoder::UiAccountEncoding::Base64),
                        data_slice: None,
                        commitment: Some(CommitmentConfig::confirmed()),
                        min_context_slot: None,
                    },
                )
                .await?;
            Ok::<_, anyhow::Error>(
                response
                    .value
                    .into_iter()
                    .enumerate()
                    .map(|(idx, v)| (chunk[idx], v.map(|account| account.data))),
            )
        });
    }

    while let Some(result) = tasks.next().await {
        accounts_vec.extend(result?);
    }
    Ok(accounts_vec)
}

async fn get_pool_program_accounts(
    rpc_client: &RpcClient,
    config: &Pubkey,
    program_id: &Pubkey,
    data_slice: Option<UiDataSliceConfig>,
) -> anyhow::Result<Vec<(Pubkey, Account)>> {
    Ok(rpc_client
        .get_program_accounts_with_config(
            program_id,
            RpcProgramAccountsConfig {
                filters: Some(vec![
                    RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
                        0,
                        PoolState::DISCRIMINATOR.to_vec(),
                    )),
                    RpcFilterType::Memcmp(Memcmp::new_raw_bytes(8, config.to_bytes().to_vec())),
                ]),
                account_config: RpcAccountInfoConfig {
                    encoding: Some(solana_account_decoder::UiAccountEncoding::Base64),
                    data_slice,
                    commitment: Some(CommitmentConfig::confirmed()),
                    min_context_slot: None,
                },
                with_context: None,
            },
        )
        .await?)
}
