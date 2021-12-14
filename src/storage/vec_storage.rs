// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::convert::Infallible;
use rkyv::{ser::Serializer, Archive, Fallible, Serialize};

use super::{Storage, UnwrapInfallible};

const PAGE_SIZE: usize = 1024 * 64;

#[derive(Debug)]
struct Page {
    bytes: Box<[u8; PAGE_SIZE]>,
    written: usize,
}

/// Storage that uses a FrozenVec of Pages to store data
#[derive(Debug)]
pub struct PageStorage {
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
    /// Creates a new empty `PageStorage`
    pub fn new() -> PageStorage {
        PageStorage { pages: vec![] }
    }
}

impl Serializer for PageStorage {
    fn pos(&self) -> usize {
        match self.pages.last() {
            None => 0,
            Some(page) => {
                let full_pages = self.pages.len() - 1;
                full_pages * PAGE_SIZE + page.written
            }
        }
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

        let page_nr = ofs as usize / PAGE_SIZE;
        let ofs = ofs as usize % PAGE_SIZE;

        let page = &self.pages[page_nr];

        let size = core::mem::size_of::<T::Archived>();
        let start = ofs as usize - size;
        let slice = &page.slice()[start..][..size];

        unsafe { rkyv::archived_root::<T>(slice) }
    }
}

#[cfg(test)]
mod tests {
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
}
