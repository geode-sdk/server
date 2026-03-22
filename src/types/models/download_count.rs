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

#[cfg(test)]
mod tests {
    use super::DownloadCount;

    #[test]
    fn serializes_as_number_by_default() {
        let serialized = serde_json::to_string(&DownloadCount::new(1234)).unwrap();

        assert_eq!(serialized, "1234");
    }

    #[test]
    fn serializes_as_abbreviated_string_when_enabled() {
        let mut count = DownloadCount::new(1234);
        count.set_abbreviated(true);
        let serialized = serde_json::to_string(&count).unwrap();

        assert_eq!(serialized, "\"1.2K\"");
    }
}
