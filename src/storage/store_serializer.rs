// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use rkyv::{ser::Serializer, Fallible, Infallible, Serialize};

use crate::{StoreProvider, StoreRef};

use super::TokenBuffer;

/// A buffered serializer wrapping a `StoreRef`
pub struct StoreSerializer<I> {
    #[allow(unused)]
    store: StoreRef<I>,
    buffer: TokenBuffer,
}

impl<I> StoreProvider<I> for StoreSerializer<I> {
    fn store(&self) -> &StoreRef<I> {
        &self.store
    }
}

impl<I> StoreSerializer<I> {
    /// Creates a new serializer from a buffer
    pub fn new(store: StoreRef<I>, buffer: TokenBuffer) -> Self {
        StoreSerializer { store, buffer }
    }

    /// Serialize into store
    pub fn serialize<T: Serialize<Self>>(&mut self, t: &T) {
        match self.serialize_value(t) {
            Ok(_) => (),
            // request more memory and retry
            Err(_) => todo!(),
        }
    }

    /// Commit the bytes written
    pub fn commit(&mut self) -> I {
        self.store.commit(&mut self.buffer)
    }
}

impl<I> Fallible for StoreSerializer<I> {
    type Error = Infallible;
}

impl<I> Serializer for StoreSerializer<I> {
    fn pos(&self) -> usize {
        self.buffer.pos()
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        loop {
            match self.buffer.write(bytes) {
                Ok(ok) => return Ok(ok),
                Err(_) => self.store.extend(&mut self.buffer),
            }
        }
    }
}

impl<I> Drop for StoreSerializer<I> {
    fn drop(&mut self) {
        let buf =
            core::mem::replace(&mut self.buffer, TokenBuffer::placeholder());
        let token = buf.consume();
        self.store.return_token(token);
    }
}
