// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::convert::Infallible;
use rkyv::ser::serializers::AllocSerializer;
use rkyv::{ser::Serializer, Archive, Fallible};

use memmap::Mmap;
use parking_lot::RwLock;

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;

use crate::Store;

use super::{Ident, Offset, StoreProvider};

const PAGE_SIZE: usize = 1024 * 64;

#[derive(Debug)]
struct Page {
    bytes: Box<[u8; PAGE_SIZE]>,
    written: usize,
}

/// Storage that uses a Vec of Pages to store data
#[derive(Debug)]
pub struct PageStorage {
    mmap: Option<Mmap>,
    file: Option<File>,
    pages: Vec<Page>,
}

impl Fallible for PageStorage {
    type Error = Infallible;
}

impl Page {
    fn new() -> Self {
        Page {
            bytes: Box::new([0u8; PAGE_SIZE]),
            written: 0,
        }
    }

    fn slice(&self) -> &[u8] {
        &self.bytes[..self.written]
    }

    fn try_write(&mut self, bytes: &[u8]) -> Result<(), usize> {
        let space_left = PAGE_SIZE - self.written;
        if space_left < bytes.len() {
            self.written += space_left;
            Err(space_left)
        } else {
            let write_into = &mut self.bytes[self.written..][..bytes.len()];
            write_into.copy_from_slice(bytes);
            self.written += bytes.len();
            Ok(())
        }
    }
}

impl PageStorage {
    const STORAGE_FILENAME: &'static str = "storage";

    /// Creates a new empty `PageStorage`
    pub fn new() -> PageStorage {
        PageStorage {
            mmap: None,
            file: None,
            pages: vec![],
        }
    }

    /// Attaches storage to a file at a given path
    fn with_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path = path.as_ref().join(Self::STORAGE_FILENAME);
        let path_exists = path.exists();
        let file = OpenOptions::new()
            .append(true)
            .read(true)
            .create(!path_exists)
            .open(&path)?;
        let mmap = if path_exists {
            Some(unsafe { Mmap::map(&file)? })
        } else {
            None
        };
        Ok(Self {
            mmap,
            file: Some(file),
            pages: Vec::new(),
        })
    }

    fn mmap_len(&self) -> usize {
        self.mmap.as_ref().map(|m| m.len()).unwrap_or(0)
    }

    fn pages_data_len(&self) -> usize {
        match self.pages.len() {
            0 => 0,
            n => {
                (n - 1) * PAGE_SIZE
                    + self.pages.last().map(|p| p.slice().len()).unwrap_or(0)
            }
        }
    }

    fn offset(&self) -> Offset {
        let mm_len = self.mmap_len();
        let data_len = self.pages_data_len();
        let ofs = mm_len + data_len;
        Offset((ofs as u64).into())
    }

    /// Persists storage to disk
    fn persist(&mut self) -> io::Result<()> {
        fn write_pages(pages: &Vec<Page>, file: &mut File) -> io::Result<()> {
            for page in pages {
                file.write(&page.bytes[..page.written])?;
            }
            file.flush()
        }
        if self.pages_data_len() > 0 {
            if let Some(file) = &mut self.file {
                write_pages(&self.pages, file)?;
                self.pages.clear();
                if self.mmap.is_none() {
                    self.mmap = Some(unsafe { Mmap::map(&file)? })
                }
            }
        }
        Ok(())
    }
}

impl PageStorage {
    fn get<T: Archive>(&self, ofs: &Offset) -> &T::Archived {
        let Offset(ofs) = *ofs;

        let size = core::mem::size_of::<T::Archived>();
        let slice = match &self.mmap {
            Some(mmap) if ofs <= mmap.len() as u64 - (size as u64) => {
                let start_pos = Into::<u64>::into(ofs) as usize;
                &mmap[start_pos..][..size]
            }
            _ => {
                let pages_ofs =
                    (Into::<u64>::into(ofs) as usize) - self.mmap_len();
                let cur_page_ofs = pages_ofs % PAGE_SIZE;
                let cur_page = pages_ofs as usize / PAGE_SIZE;

                &self.pages[cur_page].slice()[cur_page_ofs..][..size]
            }
        };
        unsafe { rkyv::archived_root::<T>(slice) }
    }

    pub fn put(&mut self, bytes: &[u8]) -> Result<Offset, Infallible> {
        let ofs = self.offset();
        if let Some(page) = self.pages.last_mut() {
            if let Ok(_) = page.try_write(bytes) {
                return Ok(ofs);
            }
        };

        self.pages.push(Page::new());

        // Re-calculate offset
        let ofs = self.offset();

        if let Some(page) = self.pages.last_mut() {
            match page.try_write(bytes) {
                Ok(()) => {
                    return Ok(ofs);
                }
                Err(_) => unreachable!(),
            }
        }
        unreachable!()
    }
}

/// Store that utilises a reference-counted PageStorage
#[derive(Clone)]
pub struct HostStore {
    inner: Arc<RwLock<PageStorage>>,
}

impl HostStore {
    /// Creates a new HostStore
    pub fn new() -> Self {
        HostStore {
            inner: Arc::new(RwLock::new(PageStorage::new())),
        }
    }
    /// Creates a new HostStore backed by a file
    pub fn with_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        Ok(HostStore {
            inner: Arc::new(RwLock::new(PageStorage::with_file(&path)?)),
        })
    }

    /// Persists storage
    pub fn persist(&mut self) -> io::Result<()> {
        self.inner.write().persist()
    }
}

impl Fallible for HostStore {
    type Error = Infallible;
}

/// Serializer running in the host context.
pub struct HostSerializer {
    store: HostStore,
    serializer: AllocSerializer<1024>,
}

impl HostSerializer {
    /// Creates a new hostserializer given a store
    pub fn new(store: HostStore) -> Self {
        HostSerializer {
            store,
            serializer: Default::default(),
        }
    }

    /// Finish the serialization and return the bytes encoded
    pub fn into_vec(self) -> Vec<u8> {
        self.serializer.into_serializer().into_inner().into()
    }
}

impl StoreProvider<HostStore> for HostSerializer {
    fn store(&self) -> &HostStore {
        &self.store
    }
}

impl Fallible for HostSerializer {
    type Error = Infallible;
}

impl Serializer for HostSerializer {
    fn pos(&self) -> usize {
        self.serializer.pos()
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        Ok(self.serializer.write(bytes).unwrap())
    }
}

impl Into<Vec<u8>> for HostSerializer {
    fn into(self) -> Vec<u8> {
        self.serializer.into_serializer().into_inner().to_vec()
    }
}

impl Store for HostStore {
    type Identifier = Offset;

    fn get<'a, T>(&'a self, id: &Ident<Self::Identifier, T>) -> &'a T::Archived
    where
        T: Archive,
    {
        let guard = self.inner.read();
        let reference = guard.get::<T>(&id.erase());
        let extended: &'a T::Archived =
            unsafe { core::mem::transmute(reference) };
        extended
    }

    fn put<T>(&self, t: &T) -> Ident<Self::Identifier, T>
    where
        T: rkyv::Serialize<Self::Serializer>,
    {
        let mut ser = HostSerializer::new(self.clone());
        let _ = ser.serialize_value(t);
        let bytes: Vec<u8> = ser.into();
        let mut guard = self.inner.write();
        let offset = guard.offset();
        guard.put(&bytes).unwrap();
        Ident::new(offset)
    }
}
