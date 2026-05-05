use sdk::service_stub::{ServiceDescriptor, run_http};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    run_http(ServiceDescriptor {
        name: settlement_worker::SERVICE_NAME,
        default_port: settlement_worker::DEFAULT_PORT,
    })
    .await
}
