// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::borrow::{Borrow, BorrowMut};
use core::marker::PhantomData;

use rkyv::{ser::Serializer, Archive, Fallible, Infallible};

pub struct Stored<T> {
    offset: u64,
    _marker: PhantomData<T>,
}

// Since `Stored` is just a wrapped u64, it is both sync and safe.
// the compiler cannot infer this without a T: Send or Sync
unsafe impl<T> Send for Stored<T> {}
unsafe impl<T> Sync for Stored<T> {}

impl<T> Clone for Stored<T> {
    fn clone(&self) -> Self {
        Stored {
            offset: self.offset,
            _marker: PhantomData,
        }
    }
}

impl<T> Copy for Stored<T> {}

impl<T> Stored<T>
where
    T: Archive,
{
    #[allow(unused)]
    pub(crate) fn new(offset: u64) -> Self {
        debug_assert!(offset % core::mem::align_of::<T>() as u64 == 0);
        Stored {
            offset,
            _marker: PhantomData,
        }
    }

    pub fn offset(self) -> u64 {
        self.offset
    }
}

impl<T> core::fmt::Debug for Stored<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Offset").field(&self.offset).finish()
    }
}

/// Portal
///
/// A hybrid memory/disk storage for an append only sequence of bytes.
#[derive(Clone, Debug, Default)]
pub struct Portal;

/// Helper trait to constrain serializers used with Storage;
pub trait StorageSerializer: Serializer + Sized + BorrowMut<Storage> {}
impl<T> StorageSerializer for T where T: Serializer + Sized + BorrowMut<Storage> {}

/// Helper trait to constrain deserializers used with Storage;
pub trait PortalDeserializer: Fallible + Sized + Borrow<Portal> {}
impl<T> PortalDeserializer for T where T: Fallible + Sized + Borrow<Portal> {}

/// Error handling
impl Fallible for Storage {
    type Error = Infallible;
}

impl Fallible for &Portal {
    type Error = Infallible;
}

#[cfg(feature = "host")]
mod host;
#[cfg(feature = "host")]
pub use host::*;

#[cfg(not(feature = "host"))]
mod guest;
#[cfg(not(feature = "host"))]
pub use guest::*;
