use alloc::sync::Arc;

use crate::id::{Id, IdHash};
use bytecheck::CheckBytes;
use rkyv::ser::{serializers::AlignedSerializer, Serializer};
use rkyv::validation::validators::DefaultValidator;
use rkyv::{
    check_archived_root, AlignedVec, Archive, Fallible, Infallible, Serialize,
};

/// The trait defining a disk or network backend for microkelvin structures.
pub trait Backend {
    /// Get get a type stored in the backend from an `Id`
    fn get(&self, id: &IdHash, len: usize) -> &[u8];

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
    pub fn get<C>(&self, id: &Id<C>) -> &C::Archived
    where
        C: Archive,
        C::Archived: for<'a> CheckBytes<DefaultValidator<'a>>,
    {
        let len = core::mem::size_of::<C::Archived>();
        let bytes = self.0.get(id.hash(), len);
        check_archived_root::<C>(bytes).expect("Invalid data")
    }

    /// Encode value into the backend, returns the Id
    pub fn put<C, S>(&self, c: &C) -> Id<C>
    where
        C: Serialize<S>,
        S: Serializer
            + Fallible
            + PortalProvider
            + From<Portal>
            + Into<AlignedVec>,
        S::Error: core::fmt::Debug,
    {
        let mut ser = S::from(self.clone());
        ser.serialize_value(c).expect("Infallible");
        let bytes = &ser.into()[..];
        let hash = IdHash::new(blake3::hash(bytes).as_bytes());
        self.0.put(hash.clone(), &bytes);
        Id::new_from_hash(hash, self.clone())
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
