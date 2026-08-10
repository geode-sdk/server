use std::{
    collections::HashSet,
    fs::File,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use crate::email::EmailAddress;

const MAX_BLOCKLIST_SIZE: u64 = 10 * 1024 * 1024; // 10 MB

#[derive(Debug, thiserror::Error)]
pub enum BlocklistError {
    #[error("blocked domain")]
    BlockedDomain,
}

/// This struct just holds an address that has been checked
/// with the blocklist.
#[derive(Clone, Debug)]
pub struct ApprovedEmailAddress(EmailAddress);

impl ApprovedEmailAddress {
    pub fn parse(
        email: EmailAddress,
        blocklist: Option<Blocklist>,
    ) -> Result<Self, BlocklistError> {
        match blocklist {
            Some(blocklist) => {
                if blocklist.is_blocked(&email.domain) {
                    Err(BlocklistError::BlockedDomain)
                } else {
                    Ok(Self(email))
                }
            }
            None => Ok(Self(email)),
        }
    }

    pub fn email(&self) -> &EmailAddress {
        &self.0
    }
}

pub struct Blocklist {
    entries: Arc<RwLock<HashSet<String>>>,
    path: Arc<PathBuf>,
}

impl Blocklist {
    pub fn load(path: PathBuf) -> Self {
        let entries = Self::read_file(&path)
            .inspect_err(|e| {
                tracing::error!("failed to parse blocklist {path}, using empty blocklist: {e}")
            })
            .unwrap_or_default();

        Self {
            entries: Arc::new(RwLock::new(entries)),
            path: Arc::new(path),
        }
    }

    pub fn is_blocked(&self, domain: &str) -> bool {
        self.entries
            .read()
            .unwrap_or_else(|poison| poison.into_inner())
            .contains(domain)
    }

    pub fn refresh(&self) {
        let entries = Self::read_file(&path).inspect_err(|e| {
            tracing::warn!("failed to parse blocklist {path}, skipping blocklist refresh: {e}")
        });

        if let Ok(entries) = entries {
            let mut guard = self
                .entries
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());

            *guard = entries;
        }
    }

    fn read_file(path: &Path) -> anyhow::Result<HashSet<String>> {
        let size = std::fs::metadata(path)?.len();

        if size > MAX_BLOCKLIST_SIZE {
            anyhow::bail!(
                "blocklist file {} is {size} bytes, exceeds {MAX_BLOCKLIST_SIZE} byte limit",
                path.display()
            )
        }

        let content = std::fs::read_to_string(path)?;

        let entries = content
            .lines()
            .map(str::trim)
            .filter(|&line| !line.is_empty() && !line.starts_with('#'))
            .map(str::to_lowercase)
            .collect();

        Ok(entries)
    }
}
