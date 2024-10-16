# gamma-swap-api
## Commands
- `cargo run use-rpc` to run in rpc-polling mode. This requires that the `RPC_NEW_POOLS_FREQUENCY` and `RPC_ACCOUNT_REFRESH_FREQUENCY` env variables be set, or passed as args with `gpa-poll-frequency-seconds` and `refresh-frequency-seconds` respectively

- `cargo run use-grpc` to run in grpc-subscription mode. This requires a compulsory `GRPC_ADDR` value and optional `GRPC_X_TOKEN` value to be present in the env or passed as args instead with `addr` and `x-token` respectively.

Flags include:
- `[Required]` The Solana Json-RPC endpoint: `--rpc-url` or `RPC_URL` in env
- `[Required]` The Amm config address: `--amm-config` or `AMM_CONFIG` in env
- `[Required]` The Amm program-id: `--amm-program-id` or `AMM_PROGRAM_ID` in env
- `[Required]` The server host configuration: `--host` or `HOST` in env
- `[Required]` The server port configuration: `--port` or `PORT` in env
- `[Required]` The blockhash poll frequency: `--blockhash-poll-frequency` or `BLOCKHASH_POLL_FREQUENCY` in env
- `[Optional]` URL to a [Quicknode-hosted](https://marketplace.quicknode.com/add-on/solana-priority-fee) priority-fee endpoint: `--priofee-url` or `PRIOFEE_URL` in env. **Note**: The binary will still run if this isn't specified, it will lack support for automatically setting priority fees on the user's transaction.
- `[Optional]` Address of the referral account for getting a share of swap fees: `--referral-account` or `REFERRAL_ACCOUNT` in env
- `[Optional]` Override the default duration(in seconds) between updating the priofee response: `priofee-poll-frequency-secs` or `PRIOFEE_POLL_FREQUENCY_SECS` in env
- `[Optional]` Override the default number of blocks considered for the priority-fee response: `priofee-n-blocks` or `PRIOFEE_N_BLOCKS` in env
- `[Optional]` Override the referral program. Gamma currently uses [this program](https://github.com/TeamRaccoons/referral.git) deployed on mainnet at [REFER4ZgmyYx9c6He5XfaTMiGfdLwRnkV4RPp9t9iF3](https://solscan.io/account/REFER4ZgmyYx9c6He5XfaTMiGfdLwRnkV4RPp9t9iF3)

## Demo
The package also includes a binary for making swaps with the http-api. First run the binary with the steps above and then `cargo run --bin swap` to make a mainnet swap for `0.01 SOL -> USDC`. This requires that a `keypair.json` file containing a funded wallet's keypair be present in the workspace root. 

Note! Low liquidity in Gamma pools atm might result in a high price-impact and less output for your trades.

- `cargo run --bin quote` to demo getting a quote from the swap API
- `cargo run --bin swap` to demo swapping 0.01 SOL for USDC using the swap API. This requires a `keypair.json` file to be present in the root

