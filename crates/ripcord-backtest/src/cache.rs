//! On-disk transaction cache.
//!
//! A backtest gets re-run dozens of times while the policy is being tuned.
//! Re-fetching a multi-hour incident window on every run burns the RPC budget
//! and makes the run slow enough to discourage iteration, so every fetched
//! transaction is written once and read thereafter. It also makes the
//! published result reproducible offline, which is the point of committing a
//! report a judge can recompute.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::Value;

pub struct Cache {
    root: PathBuf,
}

impl Cache {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)
            .with_context(|| format!("creating cache directory {}", root.display()))?;
        Ok(Self { root })
    }

    /// Sharded by the leading two characters of the signature, so no single
    /// directory holds tens of thousands of files.
    fn path_for(&self, signature: &str) -> PathBuf {
        let shard: String = signature.chars().take(2).collect();
        self.root.join(shard).join(format!("{signature}.json"))
    }

    pub fn get(&self, signature: &str) -> Option<Value> {
        let bytes = fs::read(self.path_for(signature)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn put(&self, signature: &str, value: &Value) -> Result<()> {
        let path = self.path_for(signature);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        write_atomic(&path, &serde_json::to_vec(value)?)
    }

    /// Number of cached transactions. This is the probe behind "the second run
    /// makes zero network calls".
    pub fn len(&self) -> usize {
        let Ok(shards) = fs::read_dir(&self.root) else {
            return 0;
        };
        shards
            .filter_map(Result::ok)
            .filter_map(|shard| fs::read_dir(shard.path()).ok())
            .map(|entries| entries.filter_map(Result::ok).count())
            .sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Write to a temporary file and rename, so an interrupted run never leaves a
/// truncated transaction behind that later reads as valid.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes).with_context(|| format!("writing {}", temporary.display()))?;
    fs::rename(&temporary, path).with_context(|| format!("renaming into {}", path.display()))?;
    Ok(())
}
