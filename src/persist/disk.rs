use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use appendix::Index;
use canonical::{Canon, CanonError, Id, Source};

use crate::generic::GenericTree;
use crate::persist::{Backend, PersistError, PutResult};

/// A disk-store for persisting microkelvin compound structures
pub struct DiskBackend {
    index: Index<Id, u64>,
    data_path: PathBuf,
    data_ofs: u64,
}

impl std::fmt::Debug for DiskBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DiskBackend {{ path: {:?}, data_ofs: {:?} }}",
            self.data_path, self.data_ofs
        )
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

        data.seek(SeekFrom::End(0))?;

        let data_ofs = data.metadata()?.len();

        Ok(DiskBackend {
            data_path,
            index,
            data_ofs,
        })
    }
}

impl Backend for DiskBackend {
    fn get(&self, id: &Id) -> Result<GenericTree, PersistError> {
        if let Some(ofs) = self.index.get(id)? {
            let mut data = File::open(&self.data_path)?;

            data.seek(SeekFrom::Start(*ofs))?;

            let len = id.size();

            let mut buf = Vec::with_capacity(len);
            buf.resize_with(len, || 0);

            let read_res = data.read_exact(&mut buf[..]);

            read_res?;

            let mut source = Source::new(&buf);
            Ok(GenericTree::decode(&mut source)?)
        } else {
            Err(CanonError::NotFound.into())
        }
    }

    fn put(
        &mut self,
        id: &Id,
        bytes: &[u8],
    ) -> Result<PutResult, PersistError> {
        println!("putting to disk {:?} {:?}", id, bytes);

        if self.index.get(id)?.is_some() {
            return Ok(PutResult::AlreadyPresent);
        } else {
            let data_len = id.size();
            assert_eq!(data_len, bytes.len());

            let mut data = OpenOptions::new()
                .create(true)
                .write(true)
                .append(true)
                .open(&self.data_path)?;

            data.write_all(bytes)?;

            self.index.insert(*id, self.data_ofs)?;
            self.index.flush()?;
            self.data_ofs += data_len as u64;

            Ok(PutResult::Written)
        }
    }
}
