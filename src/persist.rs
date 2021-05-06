use std::fs::{self, File, OpenOptions};
use std::io::{self, Seek, SeekFrom};
use std::path::PathBuf;

use crate::{Child, Combine, Compound, Link};
use appendix::Index;
use canonical::{Canon, CanonError, Id, Sink};
use canonical_derive::Canon;

// none [ ]
const TAG_EMPTY: u8 = 0;
// leaf [len: u16]
const TAG_LEAF: u8 = 1;
// dep [Id, annotation_len: u16]
const TAG_LINK: u8 = 2;

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

#[derive(Clone, Canon, Debug, Copy)]
pub struct Persisted(Id);

impl Persisted {
    fn restore<C, A>(self) -> Result<C, PersistError>
    where
        C: Compound<A> + Default,
        A: Annotation<C::Leaf> + Canon,
    {
        let vec: Vec<u8> = self.0.reify()?;

        let source = Source::new(&vec[..]);

        let mut compound = C::default();

        for i in 0.. {
            let tag = u8::decode(&mut source)?;

            match tag {
                TAG_END => return Ok(compound),
                TAG_EMPTY => *compound.push_empty(i)?,
                TAG_LEAF => {
                    let leaf_len = u16::decode(&mut source)?;
                    let leaf = C::Leaf::decode(&mut source)?;
                    debug_assert!(leaf.encoded_len() == leaf_len);
                    compound.push_leaf(i, leaf) = Child::Leaf(leaf);
                }
                TAG_LINK => {
                    let persisted = Persisted::decode(source)?;
                    let annotation_len = u16::decode(source)?;
                    let annotation = A::decode(source)?;
                    debug_assert!(annotation.encoded_len() == annotation_len);

                    let link = Link::from_persisted(persisted, annotation);

                    compound.push_link(i, leaf) = Child::Leaf(leaf);
                    todo!()
                }
            }
        }
    }
}

/// A disk-store for persisting microkelvin compound structures
pub struct PStore {
    path: PathBuf,
    index: Index<Id, u64>,
    data: File,
    data_ofs: u64,
}

/// The encoder for microkelvin structures
pub struct Encoder<'p> {
    store: &'p mut PStore,
    bytes: Vec<u8>,
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
        })
    }

    /// Persist a compound tree to disk
    pub fn persist<C, A>(&mut self, c: &C) -> Result<Persisted, PersistError>
    where
        C: Compound<A>,
        C::Leaf: Canon,
        A: Combine<C, A> + Canon,
    {
        let mut encoder = Encoder::new(self);

        for i in 0.. {
            match c.child(i) {
                Child::Leaf(l) => encoder.leaf::<C, A>(l),
                Child::Node(n) => encoder.link(n)?,
                Child::Empty => encoder.empty(),
                Child::EndOfNode => {
                    return Ok(encoder.end());
                }
            };
        }
        todo!()
    }
}

impl<'p> Encoder<'p> {
    fn new(store: &'p mut PStore) -> Self {
        Encoder {
            store,
            bytes: vec![],
        }
    }

    pub fn end(&mut self) -> Persisted {
        Persisted(Id::new(&self.bytes))
    }

    fn leaf<C, A>(&mut self, leaf: &C::Leaf)
    where
        C: Compound<A>,
        C::Leaf: Canon,
    {
        self.bytes.push(TAG_LEAF);
        let leaf_len = leaf.encoded_len();
        assert!(leaf_len <= core::u16::MAX as usize);
        self.bytes.push_canon(&(leaf_len as u16));
        self.bytes.push_canon(leaf);
    }

    fn empty(&mut self) {
        self.bytes.push(TAG_EMPTY);
    }

    fn link<C, A>(&mut self, dep: &Link<C, A>) -> Result<(), PersistError>
    where
        C: Compound<A>,
        C::Leaf: Canon,
        A: Combine<C, A> + Canon,
    {
        let node = &*dep.val()?;
        let anno = dep.annotation();

        let persisted = self.store.persist(node)?;

        self.bytes.push(TAG_LINK);
        self.bytes.push_canon(&persisted);

        let len = anno.encoded_len();
        assert!(len <= core::u16::MAX as usize);
        self.bytes.push_canon(&(len as u16));
        self.bytes.push_canon(anno);
        Ok(())
    }
}
