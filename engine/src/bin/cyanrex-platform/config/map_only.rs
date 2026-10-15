use serde::{
    de::{value::MapAccessDeserializer, MapAccess, Visitor},
    Deserialize, Deserializer,
};
use std::{fmt, marker::PhantomData};

// Forward maps directly, preserving duplicate keys for each strict record deserializer.
pub(super) struct MapOnly<T>(pub(super) T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for MapOnly<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Record<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for Record<T> {
            type Value = MapOnly<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object record")
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                T::deserialize(MapAccessDeserializer::new(map)).map(MapOnly)
            }
        }
        deserializer.deserialize_map(Record(PhantomData))
    }
}
