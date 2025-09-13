use serde::{Deserialize, Deserializer, Serializer};

pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&hex::encode(bytes))
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = <String>::deserialize(deserializer)?;
    hex::decode(s).map_err(serde::de::Error::custom)
}

pub mod vec_of_byte_arrays {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(vec_of_bytes: &[Vec<u8>], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hex_strings: Vec<String> =
            vec_of_bytes.iter().map(|bytes| hex::encode(bytes)).collect();
        serde::Serialize::serialize(&hex_strings, serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Vec<u8>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex_strings = <Vec<String>>::deserialize(deserializer)?;
        hex_strings
            .into_iter()
            .map(|s| hex::decode(s).map_err(serde::de::Error::custom))
            .collect()
    }
}
