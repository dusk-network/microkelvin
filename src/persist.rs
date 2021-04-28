use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use crate::{Annotation, Compound};
use appendix::Index;
use canonical::{CanonError, Id};

/// A disk-store for persisting microkelvin compound structures
pub struct PStore {
    path: PathBuf,
    index: Index<Id, u64>,
    data: File,
    data_ofs: u64,
}

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
    fn new<P>(path: P) -> io::Result<Self>
    where
        P: Into<PathBuf>,
    {
        let path = path.into();

        let mut index_path = path.clone();
        let mut data_path = path.clone();

        index_path.push("index");
        data_path.push("data");

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
}

/// The trait responsible for persisting and restoring trees to/from disk.
pub trait Persist<A>: Sized {
    /// Persist the compound structure into a persistant store
    fn persist(&self, pstore: &mut PStore) -> Result<Id, PersistError>;
    /// Restore the compound structure from a persistant store
    fn restore(id: &Id, pstore: &PStore) -> Result<Self, CanonError>;
}

impl<C, A> Persist<A> for C
where
    C: Compound<A> + Sized,
    A: Annotation<C::Leaf>,
{
    fn persist(&self, pstore: &mut PStore) -> Result<Id, PersistError> {
        todo!();
    }

    fn restore(id: &Id, pstore: &PStore) -> Result<C, CanonError> {
        todo!();
    }
}
