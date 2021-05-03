use std::fs::{self, File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::PathBuf;

use crate::{Annotated, Annotation, Child, Combine, Compound};
use appendix::Index;
use canonical::{Canon, CanonError, Id, Sink};

const TAG_END: u8 = 0;
const TAG_NONE: u8 = 1;
const TAG_LEAF: u8 = 2;
// dep signals that a dependency is to follow
const TAG_DEP: u8 = 3;
// data [len: u16]
const TAG_DATA: u8 = 4;

trait ByteVecExt {
    fn push_canon<C: Canon>(&mut self, c: &C);
}

impl ByteVecExt for Vec<u8> {
    fn push_canon<C: Canon>(&mut self, c: &C) {
        let len = c.encoded_len();
        let ofs = self.len();
        self.resize_with(ofs + len, || 0);
        let mut sink = Sink::new(&mut self[ofs..ofs + len]);
        c.encode(&mut sink);
    }
}

/// A disk-store for persisting microkelvin compound structures
pub struct PStore {
    path: PathBuf,
    index: Index<Id, u64>,
    data: File,
    data_ofs: u64,
    stack: Vec<Vec<u8>>,
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
            stack: vec![],
        })
    }

    fn push(&mut self) {
        self.stack.push(vec![])
    }

    fn pop(&mut self) -> Id {
        let bytes = self.stack.pop().expect("stack underflow");
        Id::new(&bytes)
    }

    fn top_mut(&mut self) -> &mut Vec<u8> {
        self.stack.last_mut().expect("stack underflow")
    }

    fn tag(&mut self, tag: u8) {
        self.top_mut().push(tag)
    }

    fn dep<C, A>(&mut self, dep: &Annotated<C, A>) -> Result<(), PersistError>
    where
        C: Compound<A>,
        C::Leaf: Canon,
        A: Combine<C, A> + Canon,
    {
        println!("deppo");

        let node = &*dep.val()?;
        let anno = dep.annotation();

        let id = node.persist(self)?;

        let buf = self.top_mut();
        buf.push(TAG_DEP);
        buf.push_canon(&id);
        buf.push(TAG_DATA);
        let len = anno.encoded_len();
        assert!(len <= core::u16::MAX as usize);
        buf.push_canon(&(len as u16));
        buf.push_canon(anno);
        Ok(())
    }

    fn data<C: Canon>(&mut self, c: &C) {
        let buf = self.top_mut();
        buf.push(TAG_DATA);
        let len = c.encoded_len();
        assert!(len <= core::u16::MAX as usize);
        buf.push_canon(&(len as u16));
    }
}

/// The trait responsible for persisting and restoring trees to/from disk.
pub trait Persist<A>: Sized {
    /// Persist the compound structure into a persistant store
    fn persist(&self, pstore: &mut PStore) -> Result<Id, PersistError>;
    /// Restore the compound structure from a persistant store
    fn restore(id: &Id, pstore: &PStore) -> Result<Self, PersistError>;
}

impl<C, A> Persist<A> for C
where
    C: Compound<A> + Sized,
    C::Leaf: Canon,
    A: Combine<C, A> + Canon,
{
    fn persist(&self, pstore: &mut PStore) -> Result<Id, PersistError> {
        pstore.push();

        for i in 0.. {
            match self.child(i) {
                Child::Leaf(l) => pstore.data(l),
                Child::Node(n) => pstore.dep(n)?,
                Child::Empty => pstore.tag(TAG_NONE),
                Child::EndOfNode => {
                    pstore.tag(TAG_END);
                    break;
                }
            }
        }

        Ok(pstore.pop())
    }

    fn restore(_id: &Id, _pstore: &PStore) -> Result<C, PersistError> {
        todo!();
    }
}
