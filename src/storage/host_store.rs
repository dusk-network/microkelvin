// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::convert::Infallible;

use memmap::Mmap;
use parking_lot::RwLock;
use rkyv::Fallible;

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;

use crate::Store;

use super::{OffsetLen, Token, TokenBuffer};

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
    token: Option<Token>,
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
            token: Some(Token::mint()),
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
        Ok(PageStorage {
            mmap,
            file: Some(file),
            pages: Vec::new(),
            token: Some(Token::mint()),
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
        let OffsetLen(ofs, len) = *ofs;
        let (ofs, len) = (u64::from(ofs) as usize, u16::from(len) as usize);

        let slice = match &self.mmap {
            Some(mmap) if ofs <= mmap.len() - len => &mmap[ofs..][..len],
            _ => {
                let pages_ofs = ofs - self.mmap_len();
                let cur_page_ofs = pages_ofs % PAGE_SIZE;
                let cur_page = pages_ofs / PAGE_SIZE;

                &self.pages[cur_page].slice()[cur_page_ofs..][..len]
            }
        };
        slice
    }

    fn commit(&mut self, token: Token, len: usize) -> usize {
        self.token = Some(token);
        let offset = self.offset();

        if let Some(page) = self.pages.last_mut() {
            page.commit(len)
        } else {
            // the token could not have been provided
            // unless a write-buffer was already allocated
            unreachable!()
        }
        offset
    }

    fn extend(&mut self, token: Token) {
        self.token = Some(token);
        self.pages.push(Page::new())
    }

    fn persist(&mut self) -> Result<(), std::io::Error> {
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
}

impl Store for HostStore {
    type Identifier = OffsetLen;

    fn get<'a>(&'a self, id: &Self::Identifier) -> &'a [u8] {
        let guard = self.inner.read();
        let bytes = guard.get(&id);

        let bytes_a: &'a [u8] = unsafe { core::mem::transmute(bytes) };
        bytes_a
    }

    fn write<'a>(&'a self) -> TokenBuffer<'a> {
        // loop waiting to aquire write token
        let mut guard = self.inner.write();
        let token = loop {
            if let Some(token) = guard.token.take() {
                break token;
            } else {
                guard = self.inner.write()
            }
        };

        let bytes = guard.unwritten_tail();
        let bytes_a: &'a mut [u8] = unsafe { core::mem::transmute(bytes) };

        TokenBuffer::new(token, bytes_a)
    }

    fn commit(&self, token: Token, len: usize) -> Self::Identifier {
        let mut guard = self.inner.write();
        OffsetLen(
            (guard.commit(token, len) as u64).into(),
            (len as u16).into(),
        )
    }

    fn extend(&self, token: Token) {
        let mut guard = self.inner.write();

        guard.extend(token);
    }

    fn persist(&self) -> Result<(), ()> {
        self.inner.write().persist().map_err(|_| ())
    }
}
