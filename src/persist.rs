use std::io::{self, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Arc;
use std::{
    collections::hash_map::{DefaultHasher, HashMap},
    hash::Hash,
};
use std::{
    fs::{self, File, OpenOptions},
    hash::Hasher,
};

use appendix::Index;
use canonical::{Canon, CanonError, Id};
use lazy_static::lazy_static;
use parking_lot::{Mutex, RwLock};

use crate::{Annotation, Compound};

struct WrappedBackend(Arc<RwLock<dyn Backend>>);

impl WrappedBackend {
    fn new<B: 'static + Backend>(backend: B) -> Self {
        WrappedBackend(Arc::new(RwLock::new(backend)))
    }
}

lazy_static! {
    static ref BACKENDS: Arc<RwLock<HashMap<u64, WrappedBackend>>> =
        Arc::new(RwLock::new(HashMap::new()));
}

/// A backend constructor
pub struct BackendCtor<B> {
    ctor: fn() -> B,
    id: u64,
}

impl<B> BackendCtor<B> {
    /// Create a new constructor from a function/closure
    pub fn new(ctor: fn() -> B) -> Self {
        let mut hasher = DefaultHasher::new();
        Hash::hash(&ctor, &mut hasher);
        let id = hasher.finish();
        BackendCtor { ctor, id }
    }

    fn realize(&self) -> B {
        (self.ctor)()
    }
}

/// The singleton interface to the persistance layer
pub struct Persistance;

impl Persistance {
    /// Persist the given Compound to a backend
    pub fn persist<C, A, B>(
        ctor: &BackendCtor<B>,
        c: &C,
    ) -> Result<Id, PersistError>
    where
        C: Compound<A>,
        C::Leaf: Canon,
        A: Annotation<C::Leaf> + Canon,
        B: 'static + Backend,
    {
        let mut backends = BACKENDS.write();
        backends
            .entry(ctor.id)
            .or_insert(WrappedBackend::new(ctor.realize()));
        let generic = c.generic();
        let id = Id::new(&generic);
        // DO ACTUAL WRITE
        Ok(id)
    }
}

pub trait Backend: Send + Sync {
    fn test(&mut self) {}
}

/// A disk-store for persisting microkelvin compound structures
pub struct DiskBackend {
    path: PathBuf,
    #[allow(unused)]
    index: Index<Id, u64>,
    #[allow(unused)]
    data: RwLock<File>,
    data_ofs: Mutex<u64>,
}

impl std::fmt::Debug for DiskBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DiskBackend {{ path: {:?}, data_ofs: {:?} }}",
            self.path,
            *self.data_ofs.lock()
        )
    }
}

/// An error that can appear when persisting structures to disk
#[derive(Debug)]
pub enum PersistError {
    /// An io-error occured while persisting
    Io(io::Error),
    /// A CanonError occured while persisting
    Canon(CanonError),
}

impl From<io::Error> for PersistError {
    fn from(e: io::Error) -> Self {
        PersistError::Io(e)
    }
}

impl From<CanonError> for PersistError {
    fn from(e: CanonError) -> Self {
        PersistError::Canon(e)
    }
}

impl DiskBackend {
    /// Create a new disk backend
    pub fn new<P>(path: P) -> io::Result<Self>
    where
        P: Into<PathBuf>,
    {
        let path = path.into();

        let mut index_path = path.clone();
        let mut data_path = path.clone();

        index_path.push("index");
        data_path.push("data");

        fs::create_dir(&index_path)?;

        let index = Index::new(&index_path)?;

        let mut data = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&data_path)?;

        let data_ofs = data.metadata()?.len();
        data.seek(SeekFrom::End(0))?;

        Ok(DiskBackend {
            path,
            index,
            data: RwLock::new(data),
            data_ofs: Mutex::new(data_ofs),
        })
    }
}

impl Backend for DiskBackend {
    fn test(&mut self) {}
}
