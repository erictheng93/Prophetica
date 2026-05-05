use sdk::service_stub::{ServiceDescriptor, run_http};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    run_http(ServiceDescriptor {
        name: "mock-sumsub",
        default_port: 8081,
    })
    .await
}
