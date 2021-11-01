// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::{borrow::Borrow, ops::Deref};

use rkyv::{Archive, Deserialize, Infallible};

use crate::Keyed;

/// Marker trait for types that have themselves as archived type
pub trait Primitive:
    Archive<Archived = Self> + Deserialize<Self, Infallible> + Sized
{
}

impl<T> Primitive for T where
    T: Archive<Archived = T> + Deserialize<T, Infallible> + Sized
{
}

#[derive(Debug)]
/// A wrapper around the actual type, or the archived version
pub enum AWrap<'a, T>
where
    T: Archive,
{
    /// Reference to a value
    Memory(&'a T),
    /// Reference to an archived value
    Archived(&'a T::Archived),
}

impl<'a, T> Deref for AWrap<'a, T>
where
    T: Archive,
    T::Archived: Borrow<T>,
{
    type Target = T;

    fn deref(&self) -> &T {
        match self {
            AWrap::Memory(t) => t,
            AWrap::Archived(at) => (*at).borrow(),
        }
    }
}

impl<'a, T> PartialEq<T> for AWrap<'a, T>
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

impl<'a, KV, K> Keyed<K> for AWrap<'a, KV>
where
    KV: Archive + Keyed<K>,
    KV::Archived: Keyed<K>,
{
    fn key(&self) -> &K {
        match self {
            AWrap::Memory(t) => t.key(),
            AWrap::Archived(t) => t.key(),
        }
    }
}
