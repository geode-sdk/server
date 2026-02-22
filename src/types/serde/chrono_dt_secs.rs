use chrono::{DateTime, Utc};
use serde::{self, Deserialize, Deserializer, Serializer};

pub fn serialize<S: Serializer>(dt: &DateTime<Utc>, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
}

pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<Utc>, D::Error> {
    let s = String::deserialize(d)?;
    s.parse::<DateTime<Utc>>().map_err(serde::de::Error::custom)
}

pub mod option {
    use chrono::{DateTime, Utc};
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(dt: &Option<DateTime<Utc>>, s: S) -> Result<S::Ok, S::Error> {
        match dt {
            Some(dt) => s.serialize_str(&dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<DateTime<Utc>>, D::Error> {
        let s: Option<String> = Option::deserialize(d)?;
        s.map(|s| s.parse::<DateTime<Utc>>().map_err(serde::de::Error::custom))
            .transpose()
    }
}
