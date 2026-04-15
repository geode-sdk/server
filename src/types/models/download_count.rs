use crate::abbreviate::abbreviate_number;
use serde::{Serialize, Serializer};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DownloadCount {
    count: i32,
    abbreviate: bool,
}

impl DownloadCount {
    pub const fn new(count: i32) -> Self {
        Self {
            count,
            abbreviate: false,
        }
    }

    pub fn set_abbreviated(&mut self, abbreviate: bool) {
        self.abbreviate = abbreviate;
    }
}

impl From<i32> for DownloadCount {
    fn from(count: i32) -> Self {
        Self::new(count)
    }
}

impl Serialize for DownloadCount {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.abbreviate {
            serializer.serialize_str(&abbreviate_number(self.count))
        } else {
            serializer.serialize_i32(self.count)
        }
    }
}
