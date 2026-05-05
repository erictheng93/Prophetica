use sdk::service_stub::{ServiceDescriptor, run_http};
use tokio::{runtime::Builder, task::LocalSet};

fn main() -> anyhow::Result<()> {
    sdk::observability::init_json_tracing(matching_engine::SERVICE_NAME);

    let core_ids = core_affinity::get_core_ids().unwrap_or_default();
    let pinned_core = core_ids.first().copied();

    std::thread::Builder::new()
        .name("matching-engine-market-localset".to_string())
        .spawn(move || {
            if let Some(core_id) = pinned_core {
                core_affinity::set_for_current(core_id);
                tracing::info!(core = core_id.id, "matching-engine thread pinned");
            } else {
                tracing::warn!("no CPU core id available for matching-engine pinning");
            }

            let runtime = Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(anyhow::Error::from)?;
            let local = LocalSet::new();

            local.block_on(&runtime, async {
                run_http(ServiceDescriptor {
                    name: matching_engine::SERVICE_NAME,
                    default_port: matching_engine::DEFAULT_PORT,
                })
                .await
            })
        })?
        .join()
        .map_err(|_| anyhow::anyhow!("matching-engine runtime thread panicked"))?
}
