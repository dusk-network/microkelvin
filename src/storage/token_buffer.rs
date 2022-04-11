// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use rkyv::{ser::Serializer, Fallible};
use alloc::boxed::Box;
use alloc::vec::Vec;

const UNCOMMITTED_PAGE_SIZE: usize = 1024 * 1024; // todo - needs to be elastic memory

#[derive(Debug)]
pub struct UncommittedPage {
    bytes: Box<[u8; UNCOMMITTED_PAGE_SIZE]>,
    written: usize,
}

impl UncommittedPage {
    pub fn new() -> Self {
        UncommittedPage {
            bytes: Box::new([0u8; UNCOMMITTED_PAGE_SIZE]),
            written: 0,
        }
    }
    pub fn unwritten_tail(&mut self) -> &mut [u8] {
        &mut self.bytes[self.written..]
    }
    pub fn written_slice(&self) -> &[u8] {
        &self.bytes[..self.written]
    }
    pub fn add_written(&mut self, written: usize) {
        self.written += written;
    }
    pub fn pos(&self) -> usize {
        self.written
    }
}

/// Marker type to be associated with write permissions to a backend
#[derive(Debug)]
pub enum Token {
    /// Token is active, being passed around or used to guard the write buffer
    Active,
    /// Token is somewhere else, no write permissions can be granted
    Vacant,
}

impl Token {
    /// Create a new token
    pub fn new() -> Self {
        Token::Active
    }

    /// Is the token being passed around?
    pub fn vacant(&self) -> bool {
        match self {
            Token::Active => false,
            Token::Vacant => true,
        }
    }

    /// Take the token from the slot, if any
    pub fn take(&mut self) -> Option<Token> {
        match self {
            Token::Active => {
                *self = Token::Vacant;
                Some(Token::Active)
            }
            Token::Vacant => None,
        }
    }

    /// Put the token back in its place
    pub fn return_token(&mut self, token: Token) {
        debug_assert!(self.vacant());
        debug_assert!(!token.vacant());
        *self = token;
    }
}

/// Writebuffer guarded by a `Token`
pub struct TokenBuffer {
    token: Token,
    buffer: *mut [u8],
    written: usize,
    uncommitted_pages: Vec<UncommittedPage>,
}

impl TokenBuffer {
    /// Construct a new `TokenBuffer` from a mutable slice of bytes and a token
    pub fn new(token: Token, buffer: &mut [u8]) -> Self {
        TokenBuffer {
            token,
            buffer,
            written: 0,
            uncommitted_pages: vec![],
        }
    }

    /// Construct new uncommitted
    pub fn new_uncommitted(token: Token) -> Self {
        let mut page = UncommittedPage::new();
        let buffer = page.unwritten_tail();
        TokenBuffer {
            token,
            buffer,
            written: 0,
            uncommitted_pages: vec![page],
        }
    }

    /// Reset uncommitted
    pub fn reset_uncommitted(&mut self) {
        if self.uncommitted_pages.is_empty() {
            let mut page = UncommittedPage::new();
            self.buffer = page.unwritten_tail();
            self.uncommitted_pages = vec![page];
        } else {
            for i in 1..self.uncommitted_pages.len(){
                self.uncommitted_pages.remove(i);
            }
            let mut page = self.uncommitted_pages.get_mut(0).unwrap();
            page.written = 0;
            self.buffer = page.unwritten_tail();
        }
    }

    pub(crate) fn placeholder() -> Self {
        TokenBuffer {
            token: Token::new(),
            buffer: &mut [],
            written: 0,
            uncommitted_pages: Vec::new()
        }
    }

    /// Consume the buffer, returning the held token
    pub fn consume(self) -> Token {
        self.token
    }

    /// Return bytes that have been written into the tokenbuffer
    pub fn written_bytes(&self) -> &[u8] {
        let slice = unsafe { &*self.buffer };
        &slice[..self.written]
    }

    /// Return bytes that have not yet been written
    ///
    /// # Safety
    /// It is up to the caller to assure that Only one mutable reference may
    /// exist at a time
    pub unsafe fn unwritten_bytes(&mut self) -> &mut [u8] {
        let slice = &mut *self.buffer;
        &mut slice[self.written..]
    }

    /// Temp
    pub unsafe fn last_uncommitted_slice(&self, len: usize) -> &[u8] {
        let page = self.uncommitted_pages.last().unwrap();
        &page.bytes[page.written - len..page.written]
    }

    /// Bump the buffer pointer forward, and reduce the internal count of
    /// written bytes.
    ///
    /// Returns the amount of bytes written into the lbuffer   
    pub fn advance(&mut self) -> usize {
        let written = self.written;
        self.buffer = &mut unsafe { &mut *self.buffer }[written..];
        self.written = 0;
        written
    }

    /// Remap TokenBuffer to the provided bytesg
    pub fn remap(&mut self, buffer: &mut [u8]) {
        self.buffer = buffer;
        self.written = 0;
    }

    /// Reset buffer without changing the written count (e.g. after resize)
    pub fn reset_buffer(&mut self, buffer: &mut [u8]) {
        self.buffer = buffer;
    }

    /// Rewind to the beginnging of the buffer
    pub fn rewind(&mut self) {
        self.written = 0;
    }

    /// Provide uncommitted page
    pub fn uncommitted_page(&mut self) -> &mut UncommittedPage {
        assert!(!self.uncommitted_pages.is_empty());
        self.uncommitted_pages.last_mut().unwrap()
    }
}

pub struct BufferOverflow {
    pub size_needed: usize,
}

impl BufferOverflow {
    pub fn new(size_needed: usize) -> Self {
        BufferOverflow { size_needed }
    }
}

impl Serializer for TokenBuffer {
    fn pos(&self) -> usize {
        self.written
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        let remaining_buffer_len = unsafe { self.unwritten_bytes() }.len();
        let bytes_length = bytes.len();
        if remaining_buffer_len >= bytes_length {
            let remaining_buffer = unsafe { self.unwritten_bytes() };
            remaining_buffer[..bytes_length].copy_from_slice(bytes);
            self.written += bytes_length;
            Ok(())
        } else {
            Err(BufferOverflow::new(bytes_length))
        }
    }
}

impl Fallible for TokenBuffer {
    type Error = BufferOverflow;
}

impl AsMut<[u8]> for TokenBuffer {
    fn as_mut(&mut self) -> &mut [u8] {
        unsafe { &mut *self.buffer }
    }
}
