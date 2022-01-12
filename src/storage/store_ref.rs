// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use alloc::sync::Arc;
use core::convert::Infallible;

use bytecheck::CheckBytes;
use rkyv::validation::validators::DefaultValidator;
use rkyv::{check_archived_root, Archive, Fallible, Serialize};

use crate::{Ident, Store, StoreProvider, StoreSerializer, Stored};

use super::{Token, TokenBuffer};

/// A clonable reference to a store
pub struct StoreRef<I> {
    inner: Arc<dyn Store<Identifier = I>>,
}

impl<I> StoreRef<I> {
    /// Creates a new StoreReference
    pub fn new<S: 'static + Store<Identifier = I>>(store: S) -> StoreRef<I> {
        StoreRef {
            inner: Arc::new(store),
        }
    }
}

impl<I> StoreRef<I> {
    /// Store a value, returning a `Stored` fat pointer that also carries a
    /// reference to the underlying storage with it    
    pub fn store<T>(&self, t: &T) -> Stored<T, I>
    where
        T: Serialize<StoreSerializer<I>>,
    {
        Stored::new(self.clone(), self.put(t))
    }

    /// Put a value into the store, returning an Ident.
    pub fn put<T>(&self, t: &T) -> Ident<T, I>
    where
        T: Serialize<StoreSerializer<I>>,
    {
        let mut ser = self.serializer();
        ser.serialize(t);
        let id = ser.commit();
        Ident::new(id)
    }

    /// Return a serializer assoociated with this store
    pub fn serializer(&self) -> StoreSerializer<I> {
        StoreSerializer::new(self.clone(), self.inner.request_buffer())
    }

    /// Gets a reference to an archived value
    pub fn get<T>(&self, ident: &Ident<T, I>) -> &T::Archived
    where
        T: Archive,
        T::Archived: for<'a> CheckBytes<DefaultValidator<'a>>,
    {
        let buffer = self.inner.get(ident.erase());

        let root = check_archived_root::<T>(buffer).unwrap();
        root
    }

    /// Persist the storage to a backend
    pub fn persist(&self) -> Result<(), ()> {
        self.inner.persist()
    }

    /// Commit written data, returns an identifier
    pub fn commit(&self, buffer: &mut TokenBuffer) -> I {
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

impl<I> StoreProvider<I> for StoreRef<I> {
    fn store(&self) -> &StoreRef<I> {
        self
    }
}

impl<I> Clone for StoreRef<I> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<I> Fallible for StoreRef<I> {
    type Error = Infallible;
}
