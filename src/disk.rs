use memmap::Mmap;
// use memmap::Mmap;
use parking_lot::RwLock;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

use appendix::Index;

use crate::id::IdHash;
use crate::Backend;

/// Backend for storing data on disk
pub struct DiskBackend {
    #[allow(unused)]
    path: PathBuf,
    file: RwLock<File>,
    index: Index<IdHash, u64>,
}

impl core::fmt::Debug for DiskBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiskBackend")
            .field("path", &self.path)
            .finish()
    }
}

impl DiskBackend {
    /// Create a new `DiskBackend` using path as storage.
    pub fn new<P: Into<PathBuf>>(path: P) -> Result<Self, io::Error> {
        let path = path.into();
        let data_path = path.join("data");

        let file = OpenOptions::new()
            .write(true)
            .read(false)
            .create(true)
            .open(data_path)?;

        Ok(DiskBackend {
            file: RwLock::new(file),
            index: Index::new(&path)?,
            path,
        })
    }
}

impl Backend for DiskBackend {
    fn get<Region>(&self, id: &IdHash, _len: usize) -> Region {
        let offset = self.index.get(id);

        println!("get at offset {:?}", offset);
    }

    fn put(&self, id: IdHash, serialized: &[u8]) {
        let mut file = self.file.write();
        let file_len = file.metadata().expect("file metadata error").len();
        file.write(serialized).expect("out of storage");
        self.index
            .insert(id, file_len)
            .expect("error writing to index");
    }
}
