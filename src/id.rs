// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::marker::PhantomData;

use bytecheck::CheckBytes;
use rkyv::Archive;

use crate::backend::{Getable, Portal};
use crate::error::Error;

#[derive(Debug, Clone, Hash, Copy, PartialEq, Eq, CheckBytes)]
pub struct IdHash([u8; 32]);

impl Archive for IdHash {
    type Archived = Self;

    type Resolver = Self;

    unsafe fn resolve(
        &self,
        _pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        *out = resolver
    }
}

impl From<&[u8]> for IdHash {
    fn from(_bytes: &[u8]) -> Self {
        // FIXME
        IdHash(Default::default())
    }
}

/// A marker representing a value of type `C` by hash
pub struct Id<C> {
    hash: IdHash,
    portal: Portal,
    _marker: PhantomData<C>,
}

impl<C> Clone for Id<C> {
    fn clone(&self) -> Self {
        Self {
            hash: self.hash.clone(),
            portal: self.portal.clone(),
            _marker: PhantomData,
        }
    }
}

impl<C> core::fmt::Debug for Id<C> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Id").field("hash", &self.hash).finish()
    }
}

unsafe impl<C> Send for Id<C> {}

impl<C> Id<C> {
    pub(crate) fn new_from_hash(hash: IdHash, portal: Portal) -> Self {
        Id {
            hash,
            portal,
            _marker: PhantomData,
        }
    }

    /// Pull out the represented value of the Id
    pub fn reify(&self) -> Result<C, Error>
    where
        C: Getable,
    {
        C::get(&self.hash, self.portal.clone())
    }
}
