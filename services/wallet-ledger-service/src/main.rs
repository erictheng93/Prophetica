use sdk::service_stub::{ServiceDescriptor, run_http};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    run_http(ServiceDescriptor {
        name: wallet_ledger_service::SERVICE_NAME,
        default_port: wallet_ledger_service::DEFAULT_PORT,
    })
    .await
}
