// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::hint::unreachable_unchecked;
use core::marker::PhantomData;

use rkyv::rend::LittleEndian;
use rkyv::ser::serializers::BufferSerializer;
use rkyv::{ser::Serializer, Archive, Deserialize, Fallible, Serialize};

#[cfg(feature = "host")]
mod host_store;
#[cfg(feature = "host")]
pub use host_store::{HostSerializer, HostStore};

use crate::{
    Annotation, ArchivedCompound, Branch, Compound, MaybeArchived, Walker,
};

/// Offset based identifier
#[derive(Debug, Clone, Copy, Archive, Serialize, Deserialize)]
#[archive(as = "Self")]
pub struct Offset(LittleEndian<u64>);

impl Offset {
    /// Creates an offset with a given value
    pub fn new<I: Into<LittleEndian<u64>>>(offset: I) -> Offset {
        Offset(offset.into())
    }

    /// Return the numerical offset
    pub fn inner(&self) -> u64 {
        self.0.into()
    }
}

/// An identifier representing a value stored somewhere else
pub struct Ident<I, T> {
    id: I,
    _marker: PhantomData<T>,
}

impl<I, T> core::fmt::Debug for Ident<I, T>
where
    I: core::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ident").field("id", &self.id).finish()
    }
}

impl<I, T> Clone for Ident<I, T>
where
    I: Copy,
{
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            _marker: PhantomData,
        }
    }
}

impl<I, T> Copy for Ident<I, T> where I: Copy {}

impl<I, T> Ident<I, T> {
    /// Creates a typed identifier
    pub fn new(id: I) -> Self {
        Ident {
            id,
            _marker: PhantomData,
        }
    }

    /// Returns an untyped identifier
    pub fn erase(self) -> I {
        self.id
    }
}

/// Stored is a reference to a value stored, along with the backing store
#[derive(Clone)]
pub struct Stored<T, I> {
    store: Box<dyn Store<Identifier = I>>,
    ident: Ident<I, T>,
}

unsafe impl<T, S> Send for Stored<T, S> where S: Store + Send {}
unsafe impl<T, S> Sync for Stored<T, S> where S: Store + Sync {}

impl<T, S> Stored<T, S>
where
    S: Store,
{
    /// Create a new `Stored` wrapper from an identifier and a store
    pub fn new(store: S, ident: Ident<S::Identifier, T>) -> Self {
        Stored { store, ident }
    }

    /// Get a reference to the backing Store
    pub fn store(&self) -> &S {
        &self.store
    }

    /// Get a reference to the Identifier of the stored value
    pub fn ident(&self) -> &Ident<S::Identifier, T> {
        &self.ident
    }

    /// Get a reference to the inner value being stored
    pub fn inner(&self) -> &T::Archived
    where
        T: Archive,
    {
        self.store.get(&self.ident)
    }

    /// Start a branch walk using the stored `T` as the root.  
    pub fn walk<W, A>(&self, walker: W) -> Option<Branch<T, A, S>>
    where
        S: Store,
        T: Compound<A, S>,
        T::Archived: ArchivedCompound<T, A, S>,
        T::Leaf: Archive,
        A: Annotation<T::Leaf>,
        W: Walker<T, A, S>,
    {
        let inner = self.inner();
        Branch::walk_with_store(
            MaybeArchived::Archived(inner),
            walker,
            self.store().clone(),
        )
    }
}

/// A value that carries a store with it
pub trait StoreProvider<S> {
    /// Returns a reference to the associated store
    fn store(&self) -> &S;
}

/// A type that works as a handle to a `Storage` backend.
pub trait Store: Clone + Fallible<Error = core::convert::Infallible> {
    /// The identifier used for refering to stored values
    type Identifier: Copy
        + core::fmt::Debug
        + Archive<Archived = Self::Identifier>
        + Serialize<BufferSerializer>
        + Deserialize<Self::Identifier, Self>;

    /// Put a value into storage
    fn put<T>(&self, t: &T) -> Ident<Self::Identifier, T>
    where
        T: Serialize<Self::Serializer>;

    /// Gets a reference to an archived value
    fn get<T>(&self, ident: &Ident<Self::Identifier, T>) -> &T::Archived
    where
        T: Archive;
}

/// Unwrap a result known not to have a instantiable error
pub trait UnwrapInfallible<T> {
    /// Unwrap contained value
    fn unwrap_infallible(self) -> T;
}

impl<T> UnwrapInfallible<T> for Result<T, core::convert::Infallible> {
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
