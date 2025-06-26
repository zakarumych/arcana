pub mod default_on_error {
    /// Passthrough serialization.
    #[inline(always)]
    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: serde::Serialize,
        S: serde::Serializer,
    {
        value.serialize(serializer)
    }

    /// Replaces error with default value.
    #[inline(always)]
    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: serde::Deserialize<'de> + Default,
        D: serde::Deserializer<'de>,
    {
        let value = T::deserialize(deserializer).unwrap_or_else(|_| T::default());
        Ok(value)
    }
}
