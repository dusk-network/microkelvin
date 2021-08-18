// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use appendix::Index;
use tempfile::{tempdir, TempDir};

use crate::backend::Backend;
use crate::error::Error;
use crate::id::IdHash;

/// A disk-store for persisting microkelvin compound structures
pub struct DiskBackend {
    index: Index<IdHash, u64>,
    data_path: PathBuf,
    data_ofs: AtomicU64,
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
    pub fn new<P>(path: P) -> Result<Self, io::Error>
    where
        P: Into<PathBuf>,
    {
        let path = path.into();

        let index_path = path.join("index");
        let data_path = path.join("data");

        fs::create_dir(&index_path)?;

        let index = Index::new(&index_path)?;

        let mut data = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&data_path)?;

        data.seek(SeekFrom::End(0))?;

        let data_ofs = AtomicU64::new(data.metadata()?.len());

        Ok(DiskBackend {
            data_path,
            index,
            data_ofs,
            temp_dir: None,
        })
    }

    fn register_temp_dir(&mut self, dir: TempDir) {
        self.temp_dir = Some(dir)
    }

    /// Create an ephemeral Diskbackend, that deletes its data when going out of
    /// scope
    pub fn ephemeral() -> Result<Self, io::Error> {
        let dir = tempdir()?;

        let mut db = DiskBackend::new(dir.path())?;
        db.register_temp_dir(dir);
        Ok(db)
    }

    fn get_inner(&self, id: &IdHash, into: &mut [u8]) -> Result<(), io::Error> {
        if let Some(ofs) = self.index.get(id)? {
            let mut data = File::open(&self.data_path)?;
            data.seek(SeekFrom::Start(*ofs))?;
            data.read_exact(into)
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "not found"))
        }
    }

    fn put_inner(&self, bytes: &[u8]) -> Result<IdHash, io::Error> {
        let id = IdHash::from(bytes);
        if self.index.get(&id)?.is_some() {
            return Ok(id);
        } else {
            let mut data = OpenOptions::new()
                .create(true)
                .write(true)
                .append(true)
                .open(&self.data_path)?;

            data.write_all(bytes)?;

            let len = bytes.len() as u64;
            let ofs = self.data_ofs.fetch_add(len, Ordering::SeqCst);

            self.index.insert(id, ofs)?;
            // self.index.flush()?;

            Ok(id)
        }
    }
}

impl Backend for DiskBackend {
    fn get(&self, id: &IdHash, into: &mut [u8]) -> Result<(), Error> {
        self.get_inner(id, into).map_err(|_e| Error::Missing)
    }

    fn put(&self, bytes: &[u8]) -> Result<IdHash, Error> {
        self.put_inner(bytes).map_err(|_e| Error::Missing)
    }
}
