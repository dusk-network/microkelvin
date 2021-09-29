use rkyv::Archive;

/// Marker trait for types that have themselves as archived type
pub trait Primitive: Archive<Archived = Self> + core::fmt::Debug {}

impl<T> Primitive for T where T: Archive<Archived = T> + core::fmt::Debug {}
