#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    api_gateway::run_http().await
}
