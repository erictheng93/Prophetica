use sdk::service_stub::{ServiceDescriptor, run_http};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    run_http(ServiceDescriptor {
        name: ws_gateway::SERVICE_NAME,
        default_port: ws_gateway::DEFAULT_PORT,
    })
    .await
}
