use std::{
    fs,
    path::{Path, PathBuf},
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditManifest {
    pub date: String,
    pub object_key: String,
    pub hash: String,
    pub previous_hash: String,
    pub entry_count: u64,
}

#[derive(Debug, Error)]
pub enum AuditArchiveError {
    #[error("manifest sequence is empty")]
    EmptySequence,
    #[error("manifest chain break at {date}: previous_hash {previous_hash} != {expected_hash}")]
    ChainBreak {
        date: String,
        previous_hash: String,
        expected_hash: String,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[async_trait]
pub trait AuditArchive {
    async fn put_daily_hash(
        &self,
        date: &str,
        hash: &str,
        manifest: &AuditManifest,
    ) -> Result<String, AuditArchiveError>;
}

#[derive(Debug, Clone)]
pub struct LocalAuditArchive {
    root: PathBuf,
}

impl LocalAuditArchive {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn manifest_path(&self, date: &str, hash: &str) -> PathBuf {
        self.root.join(date).join(format!("{hash}.json"))
    }

    fn object_key(date: &str, hash: &str) -> String {
        format!("wallet-ledger/daily-hashes/{date}/{hash}.json")
    }
}

#[async_trait]
impl AuditArchive for LocalAuditArchive {
    async fn put_daily_hash(
        &self,
        date: &str,
        hash: &str,
        manifest: &AuditManifest,
    ) -> Result<String, AuditArchiveError> {
        let path = self.manifest_path(date, hash);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let payload = serde_json::to_vec_pretty(manifest)?;
        fs::write(path, payload)?;

        Ok(Self::object_key(date, hash))
    }
}

pub fn load_manifest(path: &Path) -> Result<AuditManifest, AuditArchiveError> {
    let payload = fs::read(path)?;
    Ok(serde_json::from_slice(&payload)?)
}

pub fn verify_manifest_chain(manifests: &[AuditManifest]) -> Result<(), AuditArchiveError> {
    if manifests.is_empty() {
        return Err(AuditArchiveError::EmptySequence);
    }

    for pair in manifests.windows(2) {
        let previous = &pair[0];
        let current = &pair[1];
        if current.previous_hash != previous.hash {
            return Err(AuditArchiveError::ChainBreak {
                date: current.date.clone(),
                previous_hash: current.previous_hash.clone(),
                expected_hash: previous.hash.clone(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        AuditArchive, AuditArchiveError, AuditManifest, LocalAuditArchive, load_manifest,
        verify_manifest_chain,
    };

    #[tokio::test]
    async fn local_archive_writes_manifest() {
        let root = temp_root("local-archive-writes");
        let archive = LocalAuditArchive::new(&root);
        let manifest = AuditManifest {
            date: "2026-05-06".to_owned(),
            object_key: "wallet-ledger/daily-hashes/2026-05-06/hash-b.json".to_owned(),
            hash: "hash-b".to_owned(),
            previous_hash: "hash-a".to_owned(),
            entry_count: 42,
        };

        let object_key = match archive
            .put_daily_hash("2026-05-06", "hash-b", &manifest)
            .await
        {
            Ok(object_key) => object_key,
            Err(error) => panic!("manifest should write: {error}"),
        };
        let loaded = match load_manifest(&archive.manifest_path("2026-05-06", "hash-b")) {
            Ok(manifest) => manifest,
            Err(error) => panic!("manifest should load: {error}"),
        };

        assert_eq!(
            object_key,
            "wallet-ledger/daily-hashes/2026-05-06/hash-b.json"
        );
        assert_eq!(loaded, manifest);
    }

    #[test]
    fn verifies_continuous_manifest_chain() {
        let manifests = vec![
            manifest("2026-05-04", "hash-a", "genesis"),
            manifest("2026-05-05", "hash-b", "hash-a"),
            manifest("2026-05-06", "hash-c", "hash-b"),
        ];

        assert!(verify_manifest_chain(&manifests).is_ok());
    }

    #[test]
    fn rejects_broken_manifest_chain() {
        let manifests = vec![
            manifest("2026-05-04", "hash-a", "genesis"),
            manifest("2026-05-05", "hash-b", "wrong"),
        ];

        let error = match verify_manifest_chain(&manifests) {
            Ok(()) => panic!("broken chain should fail"),
            Err(error) => error,
        };

        match error {
            AuditArchiveError::ChainBreak {
                date,
                previous_hash,
                expected_hash,
            } => {
                assert_eq!(date, "2026-05-05");
                assert_eq!(previous_hash, "wrong");
                assert_eq!(expected_hash, "hash-a");
            }
            AuditArchiveError::EmptySequence
            | AuditArchiveError::Io(_)
            | AuditArchiveError::Json(_) => panic!("unexpected archive error"),
        }
    }

    fn manifest(date: &str, hash: &str, previous_hash: &str) -> AuditManifest {
        AuditManifest {
            date: date.to_owned(),
            object_key: format!("wallet-ledger/daily-hashes/{date}/{hash}.json"),
            hash: hash.to_owned(),
            previous_hash: previous_hash.to_owned(),
            entry_count: 1,
        }
    }

    fn temp_root(prefix: &str) -> std::path::PathBuf {
        let nanos = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos(),
            Err(error) => panic!("system time should be after unix epoch: {error}"),
        };
        std::env::temp_dir().join(format!("prophetica-{prefix}-{nanos}"))
    }
}
