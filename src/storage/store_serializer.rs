// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::ops::{Deref, DerefMut};

use rkyv::{
    ser::{serializers::BufferScratch, ScratchSpace, Serializer},
    Fallible, Infallible, Serialize,
};

use crate::{StoreProvider, StoreRef};

use super::TokenBuffer;

struct Buffer<B>(B);

impl<B> Deref for Buffer<B> {
    type Target = B;

    fn deref(&self) -> &B {
        &self.0
    }
}

impl<B> DerefMut for Buffer<B> {
    fn deref_mut(&mut self) -> &mut B {
        &mut self.0
    }
}

/// A buffered serializer wrapping a `StoreRef`
pub struct StoreSerializer<I> {
    #[allow(unused)]
    store: StoreRef<I>,
    buffer: TokenBuffer,
    scratch: BufferScratch<Buffer<[u8; 1024]>>,
}

impl<I> StoreProvider<I> for StoreSerializer<I> {
    fn store(&self) -> &StoreRef<I> {
        &self.store
    }
}

impl<I> StoreSerializer<I> {
    /// Creates a new serializer from a buffer
    pub fn new(store: StoreRef<I>, buffer: TokenBuffer) -> Self {
        StoreSerializer {
            store,
            buffer,
            scratch: BufferScratch::new(Buffer([0u8; 1024])),
        }
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

    /// Get access to the written bytes without writing them into the backing
    /// storage
    pub fn spill_bytes<F, R>(self, f: F) -> R
    where
        F: Fn(&[u8]) -> R,
    {
        f(self.buffer.written_bytes())
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
                Err(bo) => {
                    self.store.extend(&mut self.buffer, bo.size_needed).unwrap()
                }
            }
        }
    }
}

impl<I> ScratchSpace for StoreSerializer<I> {
    unsafe fn push_scratch(
        &mut self,
        layout: core::alloc::Layout,
    ) -> Result<core::ptr::NonNull<[u8]>, Self::Error> {
        // TODO, proper error handling
        Ok(self.scratch.push_scratch(layout).unwrap())
    }

    unsafe fn pop_scratch(
        &mut self,
        ptr: core::ptr::NonNull<u8>,
        layout: core::alloc::Layout,
    ) -> Result<(), Self::Error> {
        // TODO, proper error handling
        self.scratch.pop_scratch(ptr, layout).unwrap();
        Ok(())
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
