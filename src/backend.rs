use bytecheck::CheckBytes;
use rkyv::ser::serializers::AlignedSerializer;
use rkyv::ser::Serializer;
use rkyv::validation::validators::DefaultValidator;
use rkyv::{
    check_archived_root, AlignedVec, Archive, Deserialize, Fallible,
    Infallible, Serialize,
};

use crate::error::Error;
use crate::id::{Id, IdHash};

/// The trait defining a disk or network backend for microkelvin structures.
pub trait Backend {
    /// Get get a type stored in the backend from an `Id`
    fn get(&self, id: &IdHash, into: &mut [u8]) -> Result<(), Error>;

    /// Write encoded bytes with a corresponding `Id` into the backend
    fn put(&self, serialized: &[u8]) -> IdHash;
}

pub trait PortalProvider {
    fn portal(&self) -> &Portal;
}

pub struct PortalSerializer<S> {
    portal: Portal,
    serializer: S,
}

impl<S> Fallible for PortalSerializer<S>
where
    S: Fallible,
{
    type Error = S::Error;
}

impl<S> PortalProvider for PortalSerializer<S> {
    fn portal(&self) -> &Portal {
        &self.portal
    }
}

impl<S> Serializer for PortalSerializer<S>
where
    S: Serializer,
{
    fn pos(&self) -> usize {
        self.serializer.pos()
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), S::Error> {
        self.serializer.write(bytes)
    }
}

#[derive(Clone)]
pub struct Portal(alloc::sync::Arc<dyn Backend>);

impl Portal {
    pub(crate) fn put<C>(&self, c: &C) -> Id<C>
    where
        C: Serialize<DefaultSerializer>,
    {
        let mut ser = AlignedSerializer::new(AlignedVec::new());
        ser.serialize_value(c).expect("Infallible");
        let bytes = &ser.into_inner()[..];
        Id::new(self.0.put(bytes), self.clone())
    }

    pub(crate) fn get<C>(&self, id: &Id<C>) -> Result<C, Error>
    where
        C: Archive,
        C::Archived: Check<C>,
    {
        let mut bytes =
            vec![0u8; core::mem::size_of::<<C as Archive>::Archived>()];
        self.0.get(id.hash(), &mut bytes)?;
        let root =
            check_archived_root::<C>(&bytes).map_err(|_| Error::Invalid)?;

        root.deserialize(&mut Infallible)
            .map_err(|_| Error::Invalid)
    }
}

/// Standard microkelvin method of checking bytes and deserializing
pub trait Check<C>:
    for<'a> CheckBytes<DefaultValidator<'a>> + Deserialize<C, Infallible>
{
}

pub type DefaultSerializer = AlignedSerializer<AlignedVec>;
