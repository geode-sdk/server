use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};

use idna::{AsciiDenyList, domain_to_ascii_cow};
use parking_lot::RwLock;

use crate::email::{EmailAddress, EmailError};

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
    pub fn parse(email: EmailAddress, blocklist: Option<&Blocklist>) -> Result<Self, EmailError> {
        match blocklist {
            Some(blocklist) => {
                if blocklist.is_blocked(&email) {
                    Err(EmailError::BlocklistError(BlocklistError::BlockedDomain))
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

#[derive(Clone)]
pub struct Blocklist {
    entries: Arc<RwLock<HashSet<String>>>,
    path: Arc<PathBuf>,
}

impl Blocklist {
    pub fn load(path: PathBuf) -> Self {
        let entries = Self::read_file(&path)
            .inspect_err(|e| {
                tracing::warn!(
                    "failed to parse blocklist {}, using empty blocklist: {e}",
                    path.display()
                )
            })
            .unwrap_or_default();

        Self {
            entries: Arc::new(RwLock::new(entries)),
            path: Arc::new(path),
        }
    }

    pub fn is_blocked(&self, email: &EmailAddress) -> bool {
        let domain = email.domain.as_str();
        let mut rest = domain;

        let entries = self.entries.read();

        loop {
            if entries.contains(rest) {
                return true;
            }

            // Yeah this also checks the TLD against the blocklist, whatever
            match rest.split_once('.') {
                Some((_, parent)) => rest = parent,
                None => return false,
            }
        }
    }

    pub fn refresh(&self) {
        let entries = Self::read_file(&self.path).inspect_err(|e| {
            tracing::warn!(
                "failed to parse blocklist {}, skipping blocklist refresh: {e}",
                &self.path.display()
            )
        });

        if let Ok(entries) = entries {
            let mut guard = self.entries.write();
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

        Ok(Blocklist::parse_entries(&content))
    }

    fn parse_entries(content: &str) -> HashSet<String> {
        content
            .lines()
            .map(str::trim)
            .filter(|&line| !line.is_empty() && !line.starts_with('#'))
            .filter_map(
                |line| match domain_to_ascii_cow(line.as_bytes(), AsciiDenyList::STD3) {
                    Ok(d) => Some(d.trim_end_matches('.').to_owned()),
                    Err(_) => {
                        tracing::warn!("skipping invalid blocklist entry: {line}");
                        None
                    }
                },
            )
            .filter(|d| !d.is_empty())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use parking_lot::lock_api::RwLock;
    use std::str::FromStr;

    use super::*;

    impl Blocklist {
        fn from_str_entries(content: &str) -> Self {
            Self {
                entries: Arc::new(RwLock::new(Self::parse_entries(content))),
                path: Arc::new(PathBuf::new()),
            }
        }
    }

    #[test]
    fn test_unicode_domain_parsed_as_ascii() {
        let entries = Blocklist::parse_entries("よう.jp");

        assert!(entries.contains("xn--p8j3g.jp"));
    }

    #[test]
    fn test_domains_are_lowercased() {
        let entries = Blocklist::parse_entries("ExAmPLE.cOm");

        assert!(entries.contains("example.com"));
    }

    #[test]
    fn test_subdomain_blocked() {
        let list = Blocklist::from_str_entries("example.com");
        let email = EmailAddress::from_str("test@a.example.com").unwrap();
        assert!(list.is_blocked(&email));
    }

    #[test]
    fn test_multiple_line_blocklist() {
        let entries = Blocklist::parse_entries("example.com\ntest.com");

        assert!(entries.contains("example.com"));
        assert!(entries.contains("test.com"));
    }
}
