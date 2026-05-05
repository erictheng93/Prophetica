use sdk::service_stub::{ServiceDescriptor, run_http};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    run_http(ServiceDescriptor {
        name: user_service::SERVICE_NAME,
        default_port: user_service::DEFAULT_PORT,
    })
    .await
}
