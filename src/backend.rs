use alloc::sync::Arc;

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

pub struct PortalSerializer {
    portal: Portal,
    serializer: AlignedSerializer<AlignedVec>,
}

impl PortalSerializer {
    fn new(portal: Portal) -> Self {
        PortalSerializer {
            portal,
            serializer: Default::default(),
        }
    }

    fn into_inner(self) -> AlignedVec {
        self.serializer.into_inner()
    }
}

impl Fallible for PortalSerializer {
    type Error = Error;
}

impl PortalProvider for PortalSerializer {
    fn portal(&self) -> Portal {
        self.portal.clone()
    }
}

impl Serializer for PortalSerializer {
    fn pos(&self) -> usize {
        self.serializer.pos()
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), Error> {
        // FIXME: this error handling is non-ideal
        self.serializer.write(bytes).map_err(|_| Error::Invalid)
    }
}

/// Portal to a backend, used to erase the specific type of backend and to allow
/// efficient cloning of the reference
#[derive(Clone)]
pub struct Portal(Arc<dyn Backend>);

impl Portal {
    /// Open a new portal to a backend
    pub fn new(backend: impl Backend + 'static) -> Self {
        Portal(Arc::new(backend))
    }

    /// Get get a type stored in the backend from an `Id`
    pub fn get(&self, id: &IdHash, into: &mut [u8]) -> Result<(), Error> {
        self.0.get(id, into)
    }

    /// Write encoded bytes with a corresponding `Id` into the backend
    pub fn put(&self, serialized: &[u8]) -> Result<IdHash, Error> {
        self.0.put(serialized)
    }
}

/// Deserializer that can resolve backend values
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
pub trait Getable: Sized + Archive + Clone {
    /// Get value
    fn get(idhash: &IdHash, portal: Portal) -> Result<Self, Error>;
}

impl<C> Getable for C
where
    C: Archive + Clone,
    C::Archived: for<'a> CheckBytes<DefaultValidator<'a>>
        + Deserialize<C, PortalDeserializer>,
{
    fn get(idhash: &IdHash, portal: Portal) -> Result<Self, Error> {
        // FIXME, this could probably be changed to have the backend provide the
        // byte slice.
        let mut bytes =
            vec![0u8; core::mem::size_of::<<C as Archive>::Archived>()];
        portal.get(idhash, &mut bytes)?;

        let archived =
            check_archived_root::<C>(&bytes[..]).map_err(|_| Error::Invalid)?;

        if let Ok(val) = archived.deserialize(&mut PortalDeserializer(portal)) {
            Ok(val)
        } else {
            unreachable!()
        }
    }
}

/// Value can be put through a portal
pub trait Putable: Sized + Serialize<PortalSerializer> {
    /// Put self into the Portal, returns the generated Id
    fn put(&self, portal: Portal) -> Result<Id<Self>, Error>;
}

impl<C> Putable for C
where
    C: Serialize<PortalSerializer>,
{
    fn put(&self, portal: Portal) -> Result<Id<C>, Error> {
        let mut ser = PortalSerializer::new(portal.clone());
        ser.serialize_value(self).expect("Infallible");
        let bytes = &ser.into_inner()[..];
        let hash = portal.put(&bytes)?;
        Ok(Id::new_from_hash(hash, portal))
    }
}
