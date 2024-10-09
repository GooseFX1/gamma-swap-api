pub mod stream;

use anyhow::Context;
use geyser_grpc_connector::yellowstone_grpc_util::GeyserGrpcClientBufferConfig;
use geyser_grpc_connector::GeyserGrpcClient;
use std::time::Duration;
use yellowstone_grpc_proto::tonic::service::Interceptor;

pub async fn create_grpc_connection(
    endpoint: &String,
    x_token: &Option<String>,
) -> anyhow::Result<GeyserGrpcClient<impl Interceptor + Sized>> {
    geyser_grpc_connector::yellowstone_grpc_util::connect_with_timeout_with_buffers(
        endpoint.to_string(),
        x_token.to_owned(),
        None,
        Some(Duration::from_secs(15)),
        Some(Duration::from_secs(15)),
        GeyserGrpcClientBufferConfig::default(),
    )
    .await
    .context("Failed to connect to grpc source")
}
