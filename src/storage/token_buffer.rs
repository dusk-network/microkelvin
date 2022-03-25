// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use rkyv::{ser::Serializer, Fallible};

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
    /// temp
    pub written: usize,
    /// temp
    pub extra: usize,
}

impl TokenBuffer {
    /// Construct a new `TokenBuffer` from a mutable slice of bytes and a token
    pub fn new(token: Token, buffer: &mut [u8]) -> Self {
        TokenBuffer {
            token,
            buffer,
            written: 0,
            extra: 0,
        }
    }

    pub(crate) fn placeholder() -> Self {
        TokenBuffer {
            token: Token::new(),
            buffer: &mut [],
            written: 0,
            extra: 0,
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
            self.extra += bytes_length;
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
