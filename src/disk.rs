use memmap::MmapMut;
use parking_lot::RwLock;
use std::fs::OpenOptions;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use appendix::Index;

use crate::id::IdHash;
use crate::Backend;

const PAGE_SIZE: usize = 4096;
const MINIMAL_FREE_BYTES: usize = PAGE_SIZE * 64;

/// Backend for storing data on disk
pub struct DiskBackend {
    mmap: RwLock<MmapMut>,
    allocated_len: AtomicU64,
    written_len: AtomicU64,
    index: Index<[u8; 32], u64>,
}

impl DiskBackend {
    /// Create a new `DiskBackend` using path as storage.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, io::Error> {
        let data_path = path.as_ref().join("data");

        let file = OpenOptions::new()
            .write(true)
            .read(true)
            .create(true)
            .open(data_path)?;

        let meta = file.metadata()?;
        let len = meta.len();

        if len < MINIMAL_FREE_BYTES as u64 {
            file.set_len(MINIMAL_FREE_BYTES as u64 * 2)?;
        }

        let meta = file.metadata()?;
        let allocated_len = meta.len();

        let mmap = unsafe { MmapMut::map_mut(&file)? };

        Ok(DiskBackend {
            index: Index::new(&path)?,
            mmap: RwLock::new(mmap),
            allocated_len: AtomicU64::new(allocated_len),
            written_len: AtomicU64::new(0),
        })
    }
}

impl Backend for DiskBackend {
    fn get(&self, _id: &IdHash, _len: usize) -> &[u8] {
        todo!();
    }

    fn put(&self, id: IdHash, serialized: &[u8]) {
        let len = serialized.len();
        let ofs =
            self.written_len.fetch_add(len as u64, Ordering::SeqCst) as usize;
        let mut mmap = self.mmap.write();
        (**mmap)[ofs..][..len].copy_from_slice(serialized)
    }
}
