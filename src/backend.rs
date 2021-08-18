use bytecheck::CheckBytes;
use rkyv::ser::serializers::AlignedSerializer;
use rkyv::ser::Serializer;
use rkyv::validation::validators::DefaultValidator;
use rkyv::{
    check_archived_root, AlignedVec, Archive, Deserialize, Fallible, Serialize,
};

use crate::error::Error;
use crate::id::{Id, IdHash};

/// The trait defining a disk or network backend for microkelvin structures.
pub trait Backend {
    /// Get get a type stored in the backend from an `Id`
    fn get(&self, id: &IdHash, into: &mut [u8]) -> Result<(), Error>;

    /// Write encoded bytes with a corresponding `Id` into the backend
    fn put(&self, serialized: &[u8]) -> Result<IdHash, Error>;
}

pub trait PortalProvider {
    fn portal(&self) -> Portal;
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
    fn portal(&self) -> Portal {
        self.portal.clone()
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

/// Portal to a backend, used to erase the specific type of backend and to allow
/// efficient cloning of the reference
#[derive(Clone)]
pub struct Portal(alloc::sync::Arc<dyn Backend>);

impl Portal {
    /// Get get a type stored in the backend from an `Id`
    pub fn get(&self, id: &IdHash, into: &mut [u8]) -> Result<(), Error> {
        self.0.get(id, into)
    }

    /// Write encoded bytes with a corresponding `Id` into the backend
    pub fn put(&self, serialized: &[u8]) -> Result<IdHash, Error> {
        self.0.put(serialized)
    }
}

pub struct PortalDeserializer(Portal);

impl Fallible for PortalDeserializer {
    type Error = Error;
}

impl PortalProvider for PortalDeserializer {
    fn portal(&self) -> Portal {
        self.0.clone()
    }
}

/// This type can be parsed out of raw bytes
///
/// FIXME: naming
pub trait Getable: Sized + Archive {
    /// Get value
    fn get(idhash: &IdHash, portal: Portal) -> Result<Self, Error>;
}

pub trait Check: Archive {
    fn from_bytes(bytes: &[u8]) -> Result<&Self::Archived, Error>;
}

impl<C> Check for C
where
    C: Archive,
    C::Archived: for<'a> CheckBytes<DefaultValidator<'a>>,
{
    fn from_bytes(bytes: &[u8]) -> Result<&Self::Archived, Error> {
        debug_assert!(bytes.len() == core::mem::size_of::<C::Archived>());
        check_archived_root::<C>(&bytes[..]).map_err(|_| Error::Invalid)
    }
}

impl<C> Getable for C
where
    C: Archive + Check,
    C::Archived: Deserialize<C, PortalDeserializer>,
{
    fn get(idhash: &IdHash, portal: Portal) -> Result<Self, Error> {
        // FIXME, this could probably be changed to have the backend provide the
        // byte slice.
        let mut bytes =
            vec![0u8; core::mem::size_of::<<C as Archive>::Archived>()];
        portal.get(idhash, &mut bytes)?;

        let archived = C::from_bytes(&bytes[..])?;

        if let Ok(val) = archived.deserialize(&mut PortalDeserializer(portal)) {
            Ok(val)
        } else {
            unreachable!()
        }
    }
}

pub type DefaultSerializer = AlignedSerializer<AlignedVec>;

pub trait Putable: Sized {
    fn put(&self, portal: Portal) -> Result<Id<Self>, Error>;
}

impl<C> Putable for C
where
    C: Serialize<DefaultSerializer>,
{
    fn put(&self, portal: Portal) -> Result<Id<C>, Error> {
        let mut ser = AlignedSerializer::new(AlignedVec::new());
        ser.serialize_value(self).expect("Infallible");
        let bytes = &ser.into_inner()[..];
        let hash = portal.put(&bytes)?;
        Ok(Id::new(hash, portal))
    }
}
