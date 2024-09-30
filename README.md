# gamma-swap-api
## Commands
- `cargo run rpc` to run in rpc-polling mode. This requires that the `RPC_NEW_POOLS_FREQUENCY` and `RPC_ACCOUNT_REFRESH_FREQUENCY` env variables be set, or passed as args instead with `--gpa-poll-frequency-seconds` and `--refresh-frequency-seconds` respectively

- `cargo run grpc` to run in grpc-subscription mode. This requires a compulsory `GRPC_ADDR` value and optional `GRPC_X_TOKEN` value to be present in the env or passed as args instead with `--addr` and `--x-token` respectively.

Flags include:
- [Required] The Solana JsonRPC endpoint: `--rpc-url` or `RPC_URL` in env
- [Required] The Amm config address: `--amm-config` or `AMM_CONFIG` in env
- [Required] The Amm program-id: `--amm-program-id` or `AMM_PROGRAM_ID` in env
- [Required] The server host configuration: `--host` or `HOST` in env
- [Required] The server port configuration: `--port` or `PORT` in env
- [Required] The blockhash poll frequency: `--blockhash-poll-frequency` or `BLOCKHASH_POLL_FREQUENCY` in env


