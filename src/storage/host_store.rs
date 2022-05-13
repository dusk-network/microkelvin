// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::convert::Infallible;

use memmap2::Mmap;
use parking_lot::RwLock;
use rkyv::Fallible;

use crate::storage::PersistError;
use rkyv::ser::Serializer;
use std::fs::{File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;

use crate::Store;

use super::{OffsetLen, Token, TokenBuffer, UncommittedPage};

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
    token: Token,
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

    fn create(bytes: &[u8]) -> Self {
        let mut b = Box::new([0u8; PAGE_SIZE]);
        b[..bytes.len()].copy_from_slice(bytes);
        Page {
            bytes: b,
            written: bytes.len(),
        }
    }

    fn slice(&self) -> &[u8] {
        &self.bytes[..self.written]
    }

    fn unwritten_tail(&mut self) -> &mut [u8] {
        &mut self.bytes[self.written..]
    }

    fn commit(&mut self, len: usize) {
        self.written += len;
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
            token: Token::new(),
        }
    }

    /// Attaches storage to a file at a given path
    fn with_file<P: AsRef<Path>>(path: P) -> Result<Self, PersistError> {
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
        Ok(PageStorage {
            mmap,
            file: Some(file),
            pages: Vec::new(),
            token: Token::new(),
        })
    }

    fn mmap_len(&self) -> usize {
        self.mmap.as_ref().map(|m| m.len()).unwrap_or(0)
    }

    fn pages_data_len(&self) -> usize {
        let mut size_sum = 0;
        for p in &self.pages {
            size_sum += p.written;
        }
        size_sum
    }

    fn offset(&self) -> usize {
        self.mmap_len() + self.pages_data_len()
    }

    fn unwritten_tail<'a>(&'a mut self) -> &'a mut [u8] {
        let bytes = match self.pages.last_mut() {
            Some(page) => page.unwritten_tail(),
            None => {
                self.pages = vec![Page::new()];
                self.pages[0].unwritten_tail()
            }
        };
        let extended: &'a mut [u8] = unsafe { core::mem::transmute(bytes) };
        extended
    }

    fn get(&self, ofs: &OffsetLen) -> &[u8] {
        // generalized integer division
        fn page_for_ofs(pages_ofs: usize, pages: &Vec<Page>) -> usize {
            assert_ne!(pages.len(), 0);
            let mut sum = 0;
            for (i, p) in pages.iter().enumerate() {
                sum += p.written;
                if pages_ofs <= sum {
                    return i;
                }
            }
            unreachable!()
        }
        // generalized mod
        fn page_ofs_for_ofs(
            pages_ofs: usize,
            pages: &Vec<Page>,
            cur_page: usize,
        ) -> usize {
            assert!(cur_page < pages.len());
            if cur_page == 0 {
                pages_ofs
            } else {
                let mut sum = 0;
                for i in 0..cur_page {
                    sum += pages[i].written;
                }
                pages_ofs - sum
            }
        }
        let OffsetLen(ofs, len) = *ofs;
        let (ofs, len) = (u64::from(ofs) as usize, u32::from(len) as usize);

        let slice = match &self.mmap {
            Some(mmap) if (ofs + len) <= mmap.len() => &mmap[ofs..][..len],
            _ => {
                let pages_ofs = ofs - self.mmap_len();
                let mut cur_page = page_for_ofs(pages_ofs, &self.pages);
                let mut cur_page_ofs =
                    page_ofs_for_ofs(pages_ofs, &self.pages, cur_page);
                if page_for_ofs(pages_ofs, &self.pages)
                    < page_for_ofs(pages_ofs + len, &self.pages)
                {
                    cur_page_ofs = 0;
                    cur_page += 1;
                }

                &self.pages[cur_page].slice()[cur_page_ofs..][..len]
            }
        };
        slice
    }

    fn commit(&mut self, buffer: &mut TokenBuffer) -> OffsetLen {
        let offset = self.offset();
        let written = buffer.pos();
        buffer.uncommitted_page().add_written(written);
        buffer.advance();
        let uncommitted_len = buffer.uncommitted_len();
        if uncommitted_len <= self.unwritten_tail().len() {
            self.unwritten_tail()[..uncommitted_len].copy_from_slice(unsafe {
                buffer.last_uncommitted_slice(uncommitted_len)
            });
            if let Some(top_page) = self.pages.last_mut() {
                top_page.commit(written);
            }
        } else {
            if uncommitted_len <= PAGE_SIZE {
                self.pages.push(Page::create(
                    buffer.uncommitted_page().written_slice(),
                ));
            } else {
                self.persist(&buffer.uncommitted_pages())
                    .expect("Host store persistence");
            }
        }
        buffer.reset_uncommitted();
        OffsetLen::new(offset as u64, uncommitted_len as u32)
    }

    fn extend(
        &mut self,
        buffer: &mut TokenBuffer,
        _by: usize,
    ) -> Result<(), ()> {
        if buffer.pos() > 0 {
            buffer.extend_uncommitted();
        }
        Ok(())
    }

    fn return_token(&mut self, token: Token) {
        self.token.return_token(token)
    }

    fn persist(
        &mut self,
        uncommitted_pages: &Vec<UncommittedPage>,
    ) -> Result<(), PersistError> {
        fn write_pages(pages: &Vec<Page>, file: &mut File) -> io::Result<()> {
            for page in pages {
                file.write(&page.bytes[..page.written])?;
            }
            Ok(())
        }
        fn write_uncommitted_bytes(
            pages: &Vec<UncommittedPage>,
            file: &mut File,
        ) -> io::Result<()> {
            for p in pages {
                file.write(p.written_slice())?;
            }
            Ok(())
        }
        let mmap_len = self.mmap_len() as u64;
        if self.pages_data_len() > 0 || uncommitted_pages.len() > 0 {
            if let Some(file) = &mut self.file {
                file.seek(SeekFrom::Start(mmap_len))?;
                write_pages(&self.pages, file)?;
                write_uncommitted_bytes(uncommitted_pages, file)?;
                file.flush()?;
                self.pages.clear();
                self.mmap = Some(unsafe { Mmap::map(&*file)? })
            }
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
    /// Creates a new HostStore
    pub fn new() -> Self {
        HostStore {
            inner: Arc::new(RwLock::new(PageStorage::new())),
        }
    }

    /// Creates a new HostStore backed by a file
    pub fn with_file<P: AsRef<Path>>(path: P) -> Result<Self, PersistError> {
        Ok(HostStore {
            inner: Arc::new(RwLock::new(PageStorage::with_file(&path)?)),
        })
    }
}

impl Store for HostStore {
    type Identifier = OffsetLen;

    fn get<'a>(&'a self, id: &Self::Identifier) -> &'a [u8] {
        let guard = self.inner.read();
        let bytes = guard.get(&id);

        let bytes_a: &'a [u8] = unsafe { core::mem::transmute(bytes) };
        bytes_a
    }

    fn request_buffer(&self) -> TokenBuffer {
        // loop waiting to acquire write token
        let mut guard = self.inner.write();

        let token = loop {
            if let Some(token) = guard.token.take() {
                break token;
            } else {
                guard = self.inner.write();
            }
        };

        TokenBuffer::new_uncommitted(token)
    }

    fn persist(&self) -> Result<(), PersistError> {
        self.inner.write().persist(&vec![])
    }

    fn commit(&self, buf: &mut TokenBuffer) -> Self::Identifier {
        self.inner.write().commit(buf)
    }

    fn extend(&self, buffer: &mut TokenBuffer, by: usize) -> Result<(), ()> {
        self.inner.write().extend(buffer, by)
    }

    fn return_token(&self, token: Token) {
        self.inner.write().return_token(token)
    }
}
