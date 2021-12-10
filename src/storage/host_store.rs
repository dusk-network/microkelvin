// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::convert::Infallible;
use rkyv::{ser::Serializer, Archive, Fallible, Serialize};

use memmap::Mmap;
use parking_lot::RwLock;

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;

use crate::Store;

use super::{Ident, Offset, Storage, Stored, UnwrapInfallible};

const PAGE_SIZE: usize = 1024 * 64;

#[derive(Debug)]
struct Page {
    bytes: Box<[u8; PAGE_SIZE]>,
    written: usize,
}

#[derive(Debug)]
struct MmapStorage {
    mmap: Mmap,
    file: File,
}

/// Storage that uses a FrozenVec of Pages to store data
#[derive(Debug)]
pub struct PageStorage {
    mmap: Option<MmapStorage>,
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
        &self.bytes[..]
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
    const STORAGE_FILENAME: &'static str = "STORAGE";

    /// Creates a new empty `PageStorage`
    pub fn new() -> PageStorage {
        PageStorage {
            mmap: None,
            pages: vec![],
        }
    }

    /// Persists page storage to disk
    #[allow(dead_code)]
    fn persist<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        #[allow(unused_must_use)]
        fn write_pages(pages: &Vec<Page>, file: &mut File) -> io::Result<()> {
            for page in pages {
                file.write(&page.bytes[..page.written])?;
            }
            file.flush()
        }
        match &mut self.mmap {
            Some(mmap) => {
                write_pages(&self.pages, &mut mmap.file)?;
                self.pages.clear();
            }
            None => {
                let path = path.as_ref().join(Self::STORAGE_FILENAME);
                let mut file = OpenOptions::new()
                    .append(true)
                    .read(true)
                    .create(true)
                    .open(&path)?;
                write_pages(&self.pages, &mut file)?;
                let mmap = unsafe { Mmap::map(&file)? };
                self.mmap = Some(MmapStorage { mmap, file });
                self.pages.clear();
            }
        }
        Ok(())
    }

    /// Restore page storage from disk
    #[allow(dead_code)]
    fn restore<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path = path.as_ref().join(Self::STORAGE_FILENAME);
        if path.exists() {
            let file = OpenOptions::new()
                .append(true)
                .read(true)
                .create(false)
                .open(&path)?;

            let mmap = unsafe { Mmap::map(&file)? };
            Ok(Self {
                mmap: Some(MmapStorage { mmap, file }),
                pages: Vec::<Page>::new(),
            })
        } else {
            Ok(Self::new())
        }
    }

    fn mmap_len(&self) -> usize {
        match &self.mmap {
            Some(mmap_storage) => mmap_storage.mmap.len(),
            None => 0,
        }
    }
}

impl Serializer for PageStorage {
    fn pos(&self) -> usize {
        let pages_pos = match self.pages.last() {
            None => 0,
            Some(page) => {
                let full_pages = self.pages.len() - 1;
                full_pages * PAGE_SIZE + page.written
            }
        };
        pages_pos + self.mmap_len()
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        if let Some(page) = self.pages.last_mut() {
            if let Ok(_) = page.try_write(bytes) {
                return Ok(());
            }
        };

        self.pages.push(Page::new());

        if let Some(page) = self.pages.last_mut() {
            match page.try_write(bytes) {
                Ok(()) => {
                    return Ok(());
                }
                Err(_) => unreachable!(),
            }
        }
        unreachable!()
    }
}

impl Storage<Offset> for PageStorage {
    fn put<T: Serialize<PageStorage>>(&mut self, t: &T) -> Offset {
        self.serialize_value(t).unwrap_infallible();
        Offset(self.pos() as u64)
    }

    fn get<T: Archive>(&self, ofs: &Offset) -> &T::Archived {
        let Offset(ofs) = *ofs;
        if ofs == 0 {
            panic!("zero offset at page storage get");
        }

        let size = core::mem::size_of::<T::Archived>();
        let slice = match &self.mmap {
            Some(mmap_storage) if ofs <= mmap_storage.mmap.len() as u64 => {
                let start_pos = ofs as usize - size;
                &mmap_storage.mmap[start_pos..][..size]
            }
            _ => {
                let pages_ofs = ofs as usize - self.mmap_len();
                let cur_page_ofs = pages_ofs % PAGE_SIZE;
                let cur_page = pages_ofs as usize / PAGE_SIZE;
                let (page_nr, start_pos) = if cur_page_ofs == 0 {
                    (cur_page - 1, PAGE_SIZE - size)
                } else {
                    (cur_page, cur_page_ofs as usize - size)
                };
                &self.pages[page_nr].slice()[start_pos..][..size]
            }
        };
        unsafe { rkyv::archived_root::<T>(slice) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{HostStore, Store};
    use rkyv::rend::LittleEndian;

    #[test]
    fn it_works() {
        let store = HostStore::new();

        let a = LittleEndian::<i128>::new(8);

        let ident = store.put(&a);
        let res = ident.inner();

        assert_eq!(*res, a);
    }

    #[test]
    fn lot_more() {
        let store = HostStore::new();

        let mut ids = vec![];

        for i in 0..1024 {
            ids.push(store.put(&LittleEndian::<i128>::new(i)));
        }

        for (stored, i) in ids.iter().zip(0..) {
            let comp = LittleEndian::from(i as i128);
            let got = stored.inner();
            assert_eq!(*got, comp)
        }
    }

    #[test]
    fn many_raw_persist_and_restore() -> io::Result<()> {
        const N: usize = 1024 * 64;

        let mut references = vec![];

        let mut page_storage = PageStorage::new();

        for i in 0..N {
            let le: LittleEndian<u32> = (i as u32).into();

            references.push(page_storage.put(&le));
        }

        let le: LittleEndian<u32> = (0 as u32).into();

        assert_eq!(page_storage.get::<u32>(&references[0]), &le);

        let le: LittleEndian<u32> = (65534 as u32).into();

        assert_eq!(page_storage.get::<u32>(&references[65534]), &le);

        let le: LittleEndian<u32> = (65535 as u32).into();

        assert_eq!(page_storage.get::<u32>(&references[65535]), &le);

        for i in 0..N {
            let le: LittleEndian<u32> = (i as u32).into();

            assert_eq!(page_storage.get::<u32>(&references[i]), &le);
        }

        use tempfile::tempdir;

        let dir = tempdir()?;

        assert!(page_storage.pages.len() > 0);
        page_storage.persist(dir.path())?;
        assert_eq!(page_storage.pages.len(), 0);

        for i in 0..N {
            let le: LittleEndian<u32> = (i as u32).into();

            assert_eq!(page_storage.get::<u32>(&references[i]), &le);
        }

        // now write some more!

        for i in N..N * 2 {
            let le: LittleEndian<u32> = (i as u32).into();

            references.push(page_storage.put(&le));
        }
        assert!(page_storage.pages.len() > 0);

        // and read all back

        for i in 0..N * 2 {
            let le: LittleEndian<u32> = (i as u32).into();

            assert_eq!(page_storage.get::<u32>(&references[i]), &le);
        }

        // read all back again

        for i in 0..N * 2 {
            let le: LittleEndian<u32> = (i as u32).into();

            assert_eq!(page_storage.get::<u32>(&references[i]), &le);
        }

        // persist again and restore

        //let dir = tempdir()?; // todo it should work when persistence file is
        // changed, currently it does not

        assert!(page_storage.pages.len() > 0);
        page_storage.persist(dir.path())?;
        assert_eq!(page_storage.pages.len(), 0);

        let page_storage_restored = PageStorage::restore(dir.path())?;
        assert_eq!(page_storage_restored.pages.len(), 0);

        for i in 0..N * 2 {
            let le: LittleEndian<u32> = (i as u32).into();

            assert_eq!(page_storage_restored.get::<u32>(&references[i]), &le);
        }

        Ok(())
    }
}

/// Store that utilises a reference-counted PageStorage
#[derive(Clone)]
pub struct HostStore {
    inner: Arc<RwLock<PageStorage>>,
}

impl HostStore {
    /// Creates a new LocalStore
    pub fn new() -> Self {
        HostStore {
            inner: Arc::new(RwLock::new(PageStorage::new())),
        }
    }
}

impl Fallible for HostStore {
    type Error = Infallible;
}

impl Store for HostStore {
    type Identifier = Offset;
    type Storage = PageStorage;

    fn put<T>(&self, t: &T) -> Stored<T, Self>
    where
        T: Serialize<Self::Storage>,
    {
        Stored::new(self.clone(), Ident::new(self.inner.write().put::<T>(t)))
    }

    fn get_raw<'a, T>(
        &'a self,
        id: &Ident<Self::Identifier, T>,
    ) -> &'a T::Archived
    where
        T: Archive,
    {
        let guard = self.inner.read();
        let reference = guard.get::<T>(&id.erase());
        let extended: &'a T::Archived =
            unsafe { core::mem::transmute(reference) };
        extended
    }
}
