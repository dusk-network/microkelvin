// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::convert::Infallible;
use core::hint::unreachable_unchecked;
use core::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use bytecheck::CheckBytes;
use rkyv::{Archive, Deserialize, Fallible, Serialize};

#[cfg(feature = "host")]
mod host_store;
#[cfg(feature = "host")]
pub use host_store::HostStore;

mod store_ref;
pub use store_ref::*;

mod store_serializer;
pub use store_serializer::*;

mod token_buffer;
pub use token_buffer::*;

use crate::tower::{WellArchived, WellFormed};
use crate::{
    Annotation, ArchivedCompound, Branch, Compound, MaybeArchived, Walker,
};

/// An identifier representing a value stored somewhere else
#[derive(CheckBytes)]
pub struct Ident<T> {
    id: Identifier,
    _marker: PhantomData<T>,
}

impl<T> core::fmt::Debug for Ident<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Ident").field("id", &self.id).finish()
    }
}

impl<T> Clone for Ident<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T> Ident<T> {
    /// Creates a typed identifier
    pub fn new(id: Identifier) -> Self {
        Ident {
            id,
            _marker: PhantomData,
        }
    }

    /// Returns an untyped identifier
    pub fn erase(&self) -> &Identifier {
        &self.id
    }
}

/// Stored is a reference to a value stored, along with the backing store
#[derive(Clone, Debug)]
pub struct Stored<T> {
    store: StoreRef,
    ident: Ident<T>,
}

unsafe impl<T> Send for Stored<T> {}
unsafe impl<T> Sync for Stored<T> {}

impl<T> Stored<T> {
    /// Create a new `Stored` wrapper from an identifier and a store
    pub fn new(store: StoreRef, ident: Ident<T>) -> Self {
        Stored { store, ident }
    }

    /// Get a reference to the backing Store
    pub fn store(&self) -> &StoreRef {
        &self.store
    }

    /// Get a reference to the Identifier of the stored value
    pub fn ident(&self) -> &Ident<T> {
        &self.ident
    }

    /// Get a reference to the inner value being stored
    pub fn inner(&self) -> &T::Archived
    where
        T: WellFormed,
        T::Archived: WellArchived<T>,
    {
        self.store.get(&self.ident)
    }
}

impl<C> Stored<C> {
    /// Start a branch walk using the stored `C` as the root.
    pub fn walk<A, W>(&self, walker: W) -> Option<Branch<C, A>>
    where
        C: Compound<A> + WellFormed,
        C::Archived: ArchivedCompound<C, A> + WellArchived<C>,
        C::Leaf: 'static + WellFormed,
        <C::Leaf as Archive>::Archived: WellArchived<C::Leaf>,
        A: Annotation<C::Leaf>,
        W: Walker<C, A>,
    {
        let inner = self.inner();
        Branch::walk_with_store(
            MaybeArchived::Archived(inner),
            walker,
            self.store().clone(),
        )
    }
}

/// Trait that ensures the value has access to a store backend
pub trait StoreProvider: Sized + Fallible {
    /// Get a `StoreRef` associated with `Self`
    fn store(&self) -> &StoreRef;
}

#[derive(Clone, Archive, Serialize, Deserialize, Debug, Default)]
pub struct Identifier(Box<[u8]>);

impl<C> CheckBytes<C> for Identifier {
    type Error = core::convert::Infallible;

    unsafe fn check_bytes<'a>(
        value: *const Self,
        _: &mut C,
    ) -> Result<&'a Self, Self::Error> {
        Ok(&*value)
    }
}

impl Deref for Identifier {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Identifier {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// A type that works as a handle to a `Storage` backend.
pub trait Store {
    /// Gets a reference to an archived value
    fn get(&self, ident: &Identifier) -> &[u8];

    /// Request a buffer to write data
    fn request_buffer(&self) -> TokenBuffer;

    /// Persist to underlying storage.
    ///
    /// To keep the trait simple, the error type is omitted, and will have to be
    /// returned by other means, for example in logging.
    fn persist(&self) -> Result<(), ()>;

    /// Commit written bytes to the
    fn commit(&self, buffer: &mut TokenBuffer) -> Identifier;

    /// Request additional bytes for writing    
    fn extend(&self, buffer: &mut TokenBuffer) -> Result<(), ()>;

    /// Return the token to the store
    fn return_token(&self, token: Token);
}

/// Unwrap a result known not to have a instantiable error
pub trait UnwrapInfallible<T> {
    /// Unwrap contained value
    fn unwrap_infallible(self) -> T;
}

impl<T> UnwrapInfallible<T> for Result<T, Infallible> {
    fn unwrap_infallible(self) -> T {
        match self {
            Ok(t) => t,
            Err(_) => unsafe {
                // safe, since the error type cannot be instantiated
                unreachable_unchecked()
            },
        }
    }
}
