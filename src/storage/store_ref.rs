// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use alloc::sync::Arc;
use core::convert::Infallible;
use rkyv::ser::Serializer;

use bytecheck::CheckBytes;
use rkyv::validation::validators::DefaultValidator;
use rkyv::{check_archived_root, Archive, Fallible, Serialize};

use crate::{Ident, Store, StoreProvider, StoreSerializer, Stored};

use super::{Identifier, Token, TokenBuffer};

/// A clonable reference to a store
pub struct StoreRef {
    inner: Arc<dyn Store>,
}

impl core::fmt::Debug for StoreRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoreRef").finish()
    }
}

impl StoreRef {
    /// Creates a new StoreReference
    pub fn new<S: 'static + Store>(store: S) -> StoreRef {
        StoreRef {
            inner: Arc::new(store),
        }
    }
}

impl StoreRef {
    /// Store a value, returning a `Stored` fat pointer that also carries a
    /// reference to the underlying storage with it    
    pub fn store<T>(&self, t: &T) -> Stored<T>
    where
        T: Serialize<StoreSerializer>,
    {
        Stored::new(self.clone(), self.put(t))
    }

    /// Put a value into the store, returning an Ident.
    pub fn put<T>(&self, t: &T) -> Ident<T>
    where
        T: Serialize<StoreSerializer>,
    {
        let mut ser = self.serializer();
        ser.serialize(t);
        let id = ser.commit();
        Ident::new(id)
    }

    /// Put raw bytes into the store, returning an Identifier.
    pub fn put_raw(&self, bytes: &[u8]) -> Identifier {
        let mut ser = self.serializer();
        // write the bytes using the `Serializer` directly.
        ser.write(bytes).unwrap();
        ser.commit()
    }

    /// Return a serializer associated with this store
    pub fn serializer(&self) -> StoreSerializer {
        StoreSerializer::new(self.clone(), self.inner.request_buffer())
    }

    /// Gets a reference to an archived value
    pub fn get<T>(&self, ident: &Ident<T>) -> &T::Archived
    where
        T: Archive,
        T::Archived: for<'a> CheckBytes<DefaultValidator<'a>>,
    {
        let buffer = self.get_raw(ident.erase());

        let root = check_archived_root::<T>(buffer).unwrap();
        root
    }

    /// Gets a reference to the backing bytes of an archived value
    pub fn get_raw(&self, i: &Identifier) -> &[u8] {
        self.inner.get(i)
    }

    /// Persist the storage to a backend
    pub fn persist(&self) -> Result<(), ()> {
        self.inner.persist()
    }

    /// Commit written data, returns an identifier
    pub fn commit(&self, buffer: &mut TokenBuffer) -> Identifier {
        self.inner.commit(buffer)
    }

    /// Request extra space n the underlying buffer
    pub fn extend(&self, buffer: &mut TokenBuffer) -> Result<(), ()> {
        self.inner.extend(buffer)
    }

    /// Accept the token back
    pub fn return_token(&self, token: Token) {
        self.inner.return_token(token)
    }
}

impl StoreProvider for StoreRef {
    fn store(&self) -> &StoreRef {
        self
    }
}

impl Clone for StoreRef {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl Fallible for StoreRef {
    type Error = Infallible;
}
