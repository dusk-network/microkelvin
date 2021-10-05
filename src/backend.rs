use alloc::sync::Arc;

use crate::id::{Id, IdHash};
use rkyv::ser::{serializers::AlignedSerializer, Serializer};
use rkyv::{
    archived_root, AlignedVec, Archive, Fallible, Infallible, Serialize,
};

/// The trait defining a disk or network backend for microkelvin structures.
pub trait Backend {
    /// Get get a type stored in the backend from an `Id`
    fn get(&self, id: &IdHash, len: usize) -> [u8];

    /// Write encoded bytes into the backend
    fn put(&self, id: IdHash, serialized: &[u8]);
}

/// This type can provide a `Portal`
pub trait PortalProvider {
    /// Return a clone of the contained portal
    fn portal(&self) -> Portal;
}

pub trait IntoAlignedVec {
    fn into_inner(self) -> AlignedVec;
}

/// A serializer with access to a backend portal
#[derive(Debug)]
pub struct PortalSerializer {
    portal: Portal,
    serializer: AlignedSerializer<AlignedVec>,
}

impl Into<AlignedVec> for PortalSerializer {
    fn into(self) -> AlignedVec {
        self.serializer.into_inner()
    }
}

impl From<Portal> for PortalSerializer {
    fn from(portal: Portal) -> Self {
        PortalSerializer {
            portal,
            serializer: Default::default(),
        }
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
        self.serializer.write(bytes).expect("Infallible");
        Ok(())
    }
}

/// Portal to a backend, used to erase the specific type of backend and to allow
/// efficient cloning of the reference
#[derive(Clone)]
pub struct Portal(Arc<dyn Backend>);

impl core::fmt::Debug for Portal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Portal")
    }
}

impl Portal {
    /// Open a new portal to a backend
    pub fn new(backend: impl Backend + 'static) -> Self {
        Portal(Arc::new(backend))
    }

    /// Get get a type stored in the backend from a hash
    pub fn get<C>(&self, hash: &IdHash) -> &C::Archived
    where
        C: Archive,
    {
        let len = core::mem::size_of::<C::Archived>();
        let bytes = self.0.get(hash, len);
        // TODO: This should be using ByteCheck in the `host` version whenever
        // untrusted data is encountered
        unsafe { archived_root::<C>(bytes) }
    }

    /// Encode value into the backend, returns the Id
    pub fn put<C, S>(&self, c: &C) -> Id<C>
    where
        C: Serialize<S>,
        S: Serializer
            + Fallible
            + PortalProvider
            + Serializer
            + From<Portal>
            + Into<AlignedVec>,
    {
        let mut ser: S = From::from(self.clone());
        let _ = ser.serialize_value(c);
        let avec: AlignedVec = ser.into();
        let bytes = &avec[..];
        let hash = IdHash::new(blake3::hash(bytes).as_bytes());
        self.0.put(hash.clone(), &bytes);

        Id::new_from_hash(hash, self.clone())
    }
}

/// Deserializer that can resolve backend values
#[derive(Debug)]
pub struct PortalDeserializer(Portal);

impl Fallible for PortalDeserializer {
    type Error = Infallible;
}

impl PortalProvider for PortalDeserializer {
    fn portal(&self) -> Portal {
        self.0.clone()
    }
}

impl PortalDeserializer {
    pub(crate) fn new(portal: Portal) -> Self {
        PortalDeserializer(portal)
    }
}
