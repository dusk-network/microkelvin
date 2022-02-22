// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use appendix::Index;
use canonical::{Canon, CanonError, Id, IdHash, Source};
use parking_lot::Mutex;
use tempfile::{tempdir, TempDir};

use blake2b_simd::Params;

use crate::generic::GenericTree;
use crate::persist::{Backend, PersistError, PutResult};

/// A disk-store for persisting microkelvin compound structures
pub struct DiskBackend {
    index: Index<IdHash, (u64, u32)>,
    data_path: PathBuf,
    data_ofs: Mutex<u64>,
    // in the case of an ephemeral store, we need to extend the lifetime of the
    // `TempDir` by storing it in the struct
    #[allow(unused)]
    temp_dir: Option<TempDir>,
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
    pub fn new<P>(path: P) -> Result<Self, PersistError>
    where
        P: Into<PathBuf>,
    {
        let path = path.into();

        let index_path = path.join("index");
        let data_path = path.join("data");

        fs::create_dir_all(&index_path)?;

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
            data_ofs: Mutex::new(data_ofs),
            temp_dir: None,
        })
    }

    fn register_temp_dir(&mut self, dir: TempDir) {
        self.temp_dir = Some(dir)
    }

    /// Create an ephemeral Diskbackend, that deletes its data when going out of
    /// scope
    pub fn ephemeral() -> Result<Self, PersistError> {
        let dir = tempdir()?;

        let mut db = DiskBackend::new(dir.path())?;
        db.register_temp_dir(dir);
        Ok(db)
    }
}

impl Backend for DiskBackend {
    fn get(&self, hash: &IdHash) -> Result<GenericTree, PersistError> {
        if let Some((ofs, len)) = self.index.get(hash)? {
            let mut data = File::open(&self.data_path)?;

            data.seek(SeekFrom::Start(*ofs))?;

            let mut buf = vec![0u8; *len as usize];
            let read_res = data.read_exact(&mut buf[..]);

            read_res?;

            let mut source = Source::new(&buf);
            Ok(GenericTree::decode(&mut source)?)
        } else {
            Err(CanonError::NotFound.into())
        }
    }

    fn put(&self, bytes: &[u8]) -> Result<IdHash, PersistError> {
        let data_len = bytes.len();
        let mut state = Params::new().hash_length(32).to_state();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(state.finalize().as_ref());

        if self.index.get(&hash)?.is_some() {
            return Ok(hash);
        } else {
            let mut data = OpenOptions::new()
                .create(true)
                .write(true)
                .append(true)
                .open(&self.data_path)?;

            data.write_all(bytes)?;

            let mut data_ofs = self.data_ofs.lock();

            self.index.insert(hash, (*data_ofs, data_len as u32))?;
            // TODO make sure to flush
            // self.index.flush()?;
            *data_ofs += data_len as u64;

            Ok(hash)
        }
    }
}
