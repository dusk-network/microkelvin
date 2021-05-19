use tempfile::TempDir;

use crate::{Backend, DiskBackend, GenericTree, PersistError, PutResult};
use canonical::Id;

pub struct TestBackend(pub DiskBackend, pub TempDir);

impl TestBackend {
    pub fn new() -> Self {
        todo!()
    }
}

impl Backend for TestBackend {
    fn get(&self, id: &Id) -> Result<GenericTree, PersistError> {
        self.0.get(id)
    }

    fn put(
        &mut self,
        id: &Id,
        bytes: &[u8],
    ) -> Result<PutResult, PersistError> {
        self.0.put(id, bytes)
    }
}
