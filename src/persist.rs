use std::fs::{self, File, OpenOptions};
use std::io::{self, Seek, SeekFrom};
use std::path::PathBuf;

use appendix::Index;
use canonical::{Canon, CanonError, Id};

use crate::generic::GenericTree;
use crate::{Annotation, Compound};

/// A disk-store for persisting microkelvin compound structures
pub struct PStore {
    path: PathBuf,
    index: Index<Id, u64>,
    data: File,
    data_ofs: u64,
}

#[derive(Debug)]
pub enum PersistError {
    Io(io::Error),
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

impl PStore {
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

        Ok(PStore {
            path,
            index,
            data,
            data_ofs,
        })
    }

    /// Persist a compound tree to disk as a generic tree
    pub fn persist<C, A>(&mut self, c: &C) -> Id
    where
        C: Compound<A>,
        C::Leaf: Canon,
        A: Annotation<C::Leaf> + Canon,
    {
        let generic = c.generic();
        Id::new(&generic)
    }

    /// Restore a generic tree from storage
    pub fn restore(&self, id: Id) -> Result<GenericTree, PersistError> {
        id.reify().map_err(Into::into)
    }
}
