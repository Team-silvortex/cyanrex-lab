//! Preserve missing versus supplied records without weakening MapOnly's duplicate-key checks.
use super::MapOnly;
use serde::{Deserialize, Deserializer};

pub(super) enum Presence<T> {
    Absent,
    Present(MapOnly<T>),
}

// T need not implement Default: an omitted declaration never manufactures a default record.
impl<T> Default for Presence<T> {
    fn default() -> Self {
        Self::Absent
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Presence<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        MapOnly::<T>::deserialize(deserializer).map(Self::Present)
    }
}
