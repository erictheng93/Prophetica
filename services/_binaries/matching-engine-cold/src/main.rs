#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    sdk::observability::init_json_tracing(matching_engine::SERVICE_NAME);
    matching_engine::api::run().await
}
