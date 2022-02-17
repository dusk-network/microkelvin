// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::{borrow::Borrow, ops::Deref};

use rkyv::Archive;

use crate::storage::Stored;
use crate::Keyed;

/// Wrapper around a value either in memory or in a store
pub enum MaybeStored<'a, T> {
    /// The value is memory
    Memory(&'a T),
    /// The value is in a store
    Stored(&'a Stored<T>),
}

#[derive(Debug)]
/// A wrapper around the actual type, or the archived version
pub enum MaybeArchived<'a, T>
where
    T: Archive,
{
    /// Reference to a value
    Memory(&'a T),
    /// Reference to an archived value
    Archived(&'a T::Archived),
}

impl<'a, T> Deref for MaybeArchived<'a, T>
where
    T: Archive,
    T::Archived: Borrow<T>,
{
    type Target = T;

    fn deref(&self) -> &T {
        match self {
            MaybeArchived::Memory(t) => t,
            MaybeArchived::Archived(at) => (*at).borrow(),
        }
    }
}

impl<'a, T> PartialEq<T> for MaybeArchived<'a, T>
where
    T: Archive + PartialEq,
    T::Archived: PartialEq<T>,
{
    fn eq(&self, other: &T) -> bool {
        match (self, other) {
            (Self::Memory(l), r) => *l == r,
            (Self::Archived(l), r) => *l == r,
        }
    }
}

impl<'a, KV, K> Keyed<K> for MaybeArchived<'a, KV>
where
    KV: Archive + Keyed<K>,
    KV::Archived: Keyed<K>,
{
    fn key(&self) -> &K {
        match self {
            MaybeArchived::Memory(t) => t.key(),
            MaybeArchived::Archived(t) => t.key(),
        }
    }
}
