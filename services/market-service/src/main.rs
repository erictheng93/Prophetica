#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    sdk::observability::init_json_tracing(market_service::SERVICE_NAME);
    market_service::api::run_http().await
}
