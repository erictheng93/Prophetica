use std::{env, path::PathBuf};

use wallet_ledger_service::archive::{load_manifest, verify_manifest_chain};

fn main() -> anyhow::Result<()> {
    let paths = env::args().skip(1).map(PathBuf::from).collect::<Vec<_>>();
    if paths.is_empty() {
        anyhow::bail!("usage: verify_archive <manifest.json> [manifest.json ...]");
    }

    let mut manifests = Vec::with_capacity(paths.len());
    for path in paths {
        manifests.push(load_manifest(&path)?);
    }
    manifests.sort_by(|left, right| left.date.cmp(&right.date));
    verify_manifest_chain(&manifests)?;

    println!("verified {} audit manifests", manifests.len());
    Ok(())
}
