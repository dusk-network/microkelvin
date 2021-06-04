// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

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
