// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::ops::Deref;

use rkyv::{Archive, Deserialize, Infallible};

/// Marker trait for types that have themselves as archived type
pub trait Primitive:
    Archive<Archived = Self> + Deserialize<Self, Infallible> + Sized
{
}

impl<T> Primitive for T where
    T: Archive<Archived = T> + Deserialize<T, Infallible> + Sized
{
}

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
    T: Primitive,
{
    type Target = T;

    fn deref(&self) -> &T {
        match self {
            AWrap::Memory(t) => t,
            AWrap::Archived(at) => at,
        }
    }
}
