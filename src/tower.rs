use bytecheck::CheckBytes;
use rkyv::{
    validation::validators::DefaultValidator, Archive, Deserialize, Serialize,
};

use crate::{StoreRef, StoreSerializer};

/// A type that can be serialized and archived
pub trait WellFormed: Archive + Serialize<StoreSerializer> + Clone {}

impl<T> WellFormed for T
where
    T: Archive + Clone + Serialize<StoreSerializer>,
    T::Archived: WellArchived<T>,
{
}

/// A type that can be deserialized and checked
pub trait WellArchived<T>:
    Deserialize<T, StoreRef> + for<'a> CheckBytes<DefaultValidator<'a>>
{
}

impl<T, A> WellArchived<T> for A where
    A: Deserialize<T, StoreRef> + for<'a> CheckBytes<DefaultValidator<'a>>
{
}

/// A type that is simple and well formed
pub trait Fundamental:
    Clone
    + Archive<Archived = Self>
    + Serialize<StoreSerializer>
    + Deserialize<Self, StoreRef>
    + for<'a> CheckBytes<DefaultValidator<'a>>
{
}

impl<T> Fundamental for T where
    T: Archive<Archived = T>
        + Clone
        + Serialize<StoreSerializer>
        + Deserialize<T, StoreRef>
        + for<'a> CheckBytes<DefaultValidator<'a>>
{
}
