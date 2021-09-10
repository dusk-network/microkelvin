use alloc::sync::Arc;

use bytecheck::CheckBytes;
use rkyv::ser::serializers::AlignedSerializer;
use rkyv::ser::Serializer;
use rkyv::validation::validators::DefaultValidator;
use rkyv::{
    check_archived_root, AlignedVec, Archive, Deserialize, Fallible,
    Infallible, Serialize,
};

use crate::id::{Id, IdHash};

/// The trait defining a disk or network backend for microkelvin structures.
pub trait Backend {
    /// Get get a type stored in the backend from an `Id`
    fn get(&self, id: &IdHash, len: usize) -> &[u8];

    /// Write encoded bytes with a corresponding `Id` into the backend
    fn put(&self, serialized: &[u8]) -> IdHash;
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
    type Error = Infallible;
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

    fn write(&mut self, bytes: &[u8]) -> Result<(), <Self as Fallible>::Error> {
        Ok(self.serializer.write(bytes).expect("Infallible"))
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
    pub fn get(&self, _id: &IdHash, _len: usize) -> &[u8] {
        todo!()
    }

    /// Write encoded bytes with a corresponding `Id` into the backend
    pub fn put(&self, serialized: &[u8]) -> IdHash {
        self.0.put(serialized)
    }
}

/// Deserializer that can resolve backend values
pub struct PortalDeserializer(Portal);

impl Fallible for PortalDeserializer {
    type Error = Infallible;
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
    fn get(idhash: &IdHash, portal: Portal) -> Self;
}

impl<C> Getable for C
where
    C: Archive + Clone,
    C::Archived: for<'a> CheckBytes<DefaultValidator<'a>>
        + Deserialize<C, PortalDeserializer>,
{
    fn get(idhash: &IdHash, portal: Portal) -> Self {
        let len = core::mem::size_of::<C::Archived>();
        let pcl = portal.clone();
        let bytes = pcl.get(idhash, len);

        let archived = check_archived_root::<C>(bytes).expect("Invalid data");

        if let Ok(val) = archived.deserialize(&mut PortalDeserializer(portal)) {
            val
        } else {
            unreachable!()
        }
    }
}

/// Value can be put through a portal
pub trait Putable: Sized + Serialize<PortalSerializer> {
    /// Put self into the Portal, returns the generated Id
    fn put(&self, portal: Portal) -> Id<Self>;
}

impl<C> Putable for C
where
    C: Serialize<PortalSerializer>,
{
    fn put(&self, portal: Portal) -> Id<C> {
        let mut ser = PortalSerializer::new(portal.clone());
        ser.serialize_value(self).expect("Infallible");
        let bytes = &ser.into_inner()[..];
        let hash = portal.put(&bytes);
        Id::new_from_hash(hash, portal)
    }
}
