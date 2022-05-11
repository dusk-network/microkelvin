// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::convert::Infallible;

use memmap2::Mmap;
use parking_lot::RwLock;
use rkyv::Fallible;

use rkyv::ser::Serializer;
use std::fs::{File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;

use crate::Store;

use super::{OffsetLen, Token, TokenBuffer, UncommittedPage};

const PAGE_SIZE: usize = 1024 * 128;

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

    fn create(bytes: Box<[u8; PAGE_SIZE]>, written: usize) -> Self {
        Page { bytes, written }
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
            token: Token::new(),
        })
    }

    fn current_page(&self) -> usize {
        assert_ne!(self.pages.len(), 0);
        self.pages.len() - 1
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
        println!(
            "get ofs={} len={} number of pages={}",
            ofs.offset(),
            ofs.len(),
            &self.pages.len()
        );
        let OffsetLen(ofs, len) = *ofs;
        let (ofs, len) = (u64::from(ofs) as usize, u32::from(len) as usize);

        let slice = match &self.mmap {
            Some(mmap) if (ofs + len) <= mmap.len() => {
                println!(
                    "get from mmap ofs={} len={} .... mmaplen={}",
                    ofs,
                    len,
                    mmap.len()
                );
                &mmap[ofs..][..len]
            }
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
                println!(
                    "get - page no={} page no including len={}",
                    pages_ofs / PAGE_SIZE,
                    (pages_ofs + len) / PAGE_SIZE
                );
                println!("get - getting at pages ofs={} cur page ofs={} cur page={} len={}", pages_ofs, cur_page_ofs, cur_page, len);

                &self.pages[cur_page].slice()[cur_page_ofs..][..len]
            }
        };
        slice
    }

    fn commit(&mut self, buffer: &mut TokenBuffer) -> OffsetLen {
        println!(
            "commit num uncommitted pages={} buffer pos={}",
            buffer.uncomitted_pages.len(),
            buffer.pos()
        );
        let offset = self.offset();
        let written = buffer.pos();
        if let Some(top_uncomitted_page) = buffer.uncomitted_pages.last_mut() {
            top_uncomitted_page.written = written;
        }
        buffer.advance();
        let mut uncommitted_len = 0;
        for p in &buffer.uncomitted_pages {
            self.pages.push(Page::create(p.bytes.clone(), p.written));
            uncommitted_len += p.written;
            println!("commit written={}", p.written);
        }
        buffer.uncomitted_pages = Vec::new();

        // if let Some(page) = self.pages.last_mut() {
        //     page.written += advance_len - original_extra;
        // } else {
        // the token could not have been provided
        // unless a write-buffer was already allocated
        // unreachable!()
        // }
        println!("commit returning offs={} len={}", offset, uncommitted_len);
        OffsetLen::new(offset as u64, uncommitted_len as u32)
    }

    fn extend(
        &mut self,
        buffer: &mut TokenBuffer,
        by: usize,
    ) -> Result<(), ()> {
        println!("extend");
        let mut clear_written = false;
        // if let Some(current_page) = self.pages.last() {
        //     if (current_page.written + buffer.pos() + by) > PAGE_SIZE {
        //         println!("extend buffer pos={} by={} s={}", buffer.pos(), by,
        // current_page.written);         clear_written = true;
        //     }
        // }
        let mut new_uncomitted_page = UncommittedPage::new();
        if buffer.pos() > 0 {
            new_uncomitted_page.unwritten_tail()[..buffer.pos()]
                .copy_from_slice(buffer.written_bytes());
            new_uncomitted_page.written = buffer.pos();
            buffer.reset_buffer(new_uncomitted_page.unwritten_tail());
            buffer.uncomitted_pages.push(new_uncomitted_page);
            println!("quasi commit of {}", buffer.pos());
        }
        // else {
        //     println!("pos0 written={} unwritten={} by={} extra={}",
        // buffer.written_bytes().len(), unsafe { buffer.unwritten_bytes().len()
        // }, by, buffer.extra );     self.pages.push(Page::new());
        //     buffer.remap(self.unwritten_tail());
        // }
        // if clear_written {
        //     println!("abcd zeroing {}", buffer.pos());
        //     buffer.extra += buffer.pos();
        //     buffer.written = 0;
        // }
        Ok(())
    }

    fn return_token(&mut self, token: Token) {
        self.token.return_token(token)
    }

    fn persist(&mut self) -> Result<(), std::io::Error> {
        fn write_pages(pages: &Vec<Page>, file: &mut File) -> io::Result<()> {
            for page in pages {
                println!("persisting page {}", page.written);
                file.write(&page.bytes[..page.written])?;
            }
            file.flush()
        }
        println!(
            "persist: data in pages={} data in mmap={}",
            self.pages_data_len(),
            self.mmap_len()
        );
        let mmap_len = self.mmap_len() as u64;
        if self.pages_data_len() > 0 {
            if let Some(file) = &mut self.file {
                println!("seek to {}", mmap_len);
                file.seek(SeekFrom::Start(mmap_len));
                write_pages(&self.pages, file)?;
                file.seek(SeekFrom::Start(0));
                self.pages.clear();
                // if self.mmap.is_none() {
                self.mmap = Some(unsafe { Mmap::map(&*file)? })
                // }
            }
        }
        println!(
            "persist after: data in pages={} data in mmap={}",
            self.pages_data_len(),
            self.mmap_len()
        );
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
        println!("before transmute");
        let bytes_a: &'a [u8] = unsafe { core::mem::transmute(bytes) };
        println!("after transmute");
        bytes_a
    }

    fn request_buffer(&self) -> TokenBuffer {
        // loop waiting to aquire write token
        let mut guard = self.inner.write();

        let token = loop {
            if let Some(token) = guard.token.take() {
                break token;
            } else {
                guard = self.inner.write();
            }
        };

        // let bytes = guard.unwritten_tail();
        TokenBuffer::new_uncommitted(token)
    }

    fn extend(
        &self,
        buffer: &mut TokenBuffer,
        size_needed: usize,
    ) -> Result<(), ()> {
        self.inner.write().extend(buffer, size_needed)
    }

    fn persist(&self) -> Result<(), ()> {
        self.inner.write().persist().map_err(|_| ())
    }

    fn commit(&self, buf: &mut TokenBuffer) -> Self::Identifier {
        self.inner.write().commit(buf)
    }

    fn return_token(&self, token: Token) {
        self.inner.write().return_token(token)
    }
}
