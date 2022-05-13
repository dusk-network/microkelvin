// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::convert::Infallible;
use core::hint::unreachable_unchecked;
use core::marker::PhantomData;

use bytecheck::CheckBytes;
use rkyv::rend::LittleEndian;
use rkyv::validation::validators::DefaultValidator;
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

mod error;
pub use error::*;

use crate::{
    Annotation, ArchivedCompound, Branch, Compound, MaybeArchived, Walker,
};

/// Offset based identifier
#[derive(
    Debug, Clone, Copy, Archive, Serialize, Deserialize, CheckBytes, Default,
)]
#[archive(as = "Self")]
pub struct OffsetLen(LittleEndian<u64>, LittleEndian<u32>);

impl OffsetLen {
    /// Creates an offset with a given value
    pub fn new<O: Into<LittleEndian<u64>>, L: Into<LittleEndian<u32>>>(
        offset: O,
        len: L,
    ) -> OffsetLen {
        OffsetLen(offset.into(), len.into())
    }

    /// Return the numerical offset
    pub fn inner(&self) -> u64 {
        self.0.into()
    }

    /// The offset in storage
    pub fn offset(&self) -> u64 {
        u64::from(self.0)
    }

    /// The length of the byte representation
    pub fn len(&self) -> u32 {
        u32::from(self.1)
    }
}

/// An identifier representing a value stored somewhere else
#[derive(CheckBytes)]
pub struct Ident<T, I> {
    id: I,
    _marker: PhantomData<T>,
}

impl<T, I> core::fmt::Debug for Ident<T, I>
where
    I: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Ident").field("id", &self.id).finish()
    }
}

impl<T, I> Clone for Ident<T, I>
where
    I: Clone,
{
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T, I> Copy for Ident<T, I> where I: Copy {}

impl<T, I> Ident<T, I> {
    /// Creates a typed identifier
    pub fn new(id: I) -> Self {
        Ident {
            id,
            _marker: PhantomData,
        }
    }

    /// Returns an untyped identifier
    pub fn erase(&self) -> &I {
        &self.id
    }
}

/// Stored is a reference to a value stored, along with the backing store
#[derive(Clone)]
pub struct Stored<T, I> {
    store: StoreRef<I>,
    ident: Ident<T, I>,
}

unsafe impl<T, I> Send for Stored<T, I> where I: Send {}
unsafe impl<T, I> Sync for Stored<T, I> where I: Sync {}

impl<T, I> Stored<T, I> {
    /// Create a new `Stored` wrapper from an identifier and a store
    pub fn new(store: StoreRef<I>, ident: Ident<T, I>) -> Self {
        Stored { store, ident }
    }

    /// Get a reference to the backing Store
    pub fn store(&self) -> &StoreRef<I> {
        &self.store
    }

    /// Get a reference to the Identifier of the stored value
    pub fn ident(&self) -> &Ident<T, I> {
        &self.ident
    }

    /// Get a reference to the inner value being stored
    pub fn inner(&self) -> &T::Archived
    where
        T: Archive,
        T::Archived: for<'a> CheckBytes<DefaultValidator<'a>>,
    {
        self.store.get(&self.ident)
    }

    /// Start a branch walk using the stored `T` as the root.  
    pub fn walk<W, A>(&self, walker: W) -> Option<Branch<T, A, I>>
    where
        T: Compound<A, I>,
        T::Archived: ArchivedCompound<T, A, I>
            + for<'any> CheckBytes<DefaultValidator<'any>>,
        T::Leaf: 'static + Archive,
        A: Annotation<T::Leaf>,
        W: Walker<T, A, I>,
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
pub trait StoreProvider<I>: Sized + Fallible {
    /// Get a `StoreRef` associated with `Self`
    fn store(&self) -> &StoreRef<I>;
}

/// A type that works as a handle to a `Storage` backend.
pub trait Store {
    /// The identifier used for refering to stored values
    type Identifier;

    /// Gets a reference to an archived value
    fn get(&self, ident: &Self::Identifier) -> &[u8];

    /// Request a buffer to write data
    fn request_buffer(&self) -> TokenBuffer;

    /// Persist to underlying storage.
    ///
    /// To keep the trait simple, the error type is omitted, and will have to be
    /// returned by other means, for example in logging.
    fn persist(&self) -> Result<(), ()>;

    /// Commit written bytes to the
    fn commit(&self, buffer: &mut TokenBuffer) -> Self::Identifier;

    /// Request additional bytes for writing
    fn extend(
        &self,
        buffer: &mut TokenBuffer,
        size_needed: usize,
    ) -> Result<(), ()>;

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
