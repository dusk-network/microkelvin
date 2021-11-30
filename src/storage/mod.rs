// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::hint::unreachable_unchecked;
use core::marker::PhantomData;

use rkyv::{ser::Serializer, Archive, Fallible, Serialize};

#[cfg(feature = "host")]
mod host_store;
#[cfg(feature = "host")]
pub use host_store::HostStore;

use crate::{
    Annotation, ArchivedCompound, Branch, Compound, MaybeArchived, Walker,
};

pub struct Ident<I, T> {
    id: I,
    _marker: PhantomData<T>,
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
    pub(crate) fn new(id: I) -> Self {
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
pub struct Stored<T, S>
where
    S: Store,
{
    store: S,
    ident: Ident<S::Identifier, T>,
}

unsafe impl<T, S> Send for Stored<T, S> where S: Store + Send {}
unsafe impl<T, S> Sync for Stored<T, S> where S: Store + Sync {}

impl<T, S> Stored<T, S>
where
    S: Store,
{
    pub(crate) fn new(store: S, ident: Ident<S::Identifier, T>) -> Self {
        Stored { store, ident }
    }

    /// Get a reference to the underlying store
    pub fn store(&self) -> &S {
        &self.store
    }

    /// Get a reference to the underlying identifier
    pub fn ident(&self) -> &Ident<S::Identifier, T> {
        &self.ident
    }

    /// The archived value
    pub fn inner(&self) -> &T::Archived
    where
        T: Archive,
    {
        self.store.get_raw(&self.ident)
    }

    /// Start a walk at the stored value
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

/// A type that works as a handle to a `Storage` backend.
pub trait Store: Clone + Fallible<Error = core::convert::Infallible> {
    /// The identifier used for refering to stored values
    type Identifier: Copy;
    /// The underlying storage
    type Storage: Storage<Self::Identifier>;

    /// Put a value into storage, and get a representative token back
    fn put<T>(&self, t: &T) -> Stored<T, Self>
    where
        T: Serialize<Self::Storage>;

    /// Gets a reference to an archived value
    fn get_raw<'a, T>(
        &'a self,
        ident: &Ident<Self::Identifier, T>,
    ) -> &'a T::Archived
    where
        T: Archive;
}

/// The main trait for providing storage backends to use with `microkelvin`
pub trait Storage<I>:
    Serializer + Fallible<Error = std::convert::Infallible>
{
    /// Write a value into the storage, returns a representation
    fn put<T>(&mut self, t: &T) -> I
    where
        T: Serialize<Self>;

    /// Gets a value from the store
    fn get<T>(&self, id: &I) -> &T::Archived
    where
        T: Archive;
}

pub trait UnwrapInfallible<T> {
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
