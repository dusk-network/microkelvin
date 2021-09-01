// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::marker::PhantomData;

use rkyv::Archive;

use crate::backend::{Check, Portal};
use crate::error::Error;

#[derive(Debug, Clone, Hash, Copy, PartialEq, Eq)]
pub struct IdHash([u8; 32]);

impl From<&[u8]> for IdHash {
    fn from(_bytes: &[u8]) -> Self {
        IdHash(Default::default())
    }
}

#[derive(Clone)]
pub struct Id<C> {
    hash: IdHash,
    portal: Portal,
    _marker: PhantomData<C>,
}

impl<C> core::fmt::Debug for Id<C> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Id").field("hash", &self.hash).finish()
    }
}

unsafe impl<C> Send for Id<C> {}

impl<C> Id<C> {
    pub(crate) fn new(hash: IdHash, portal: Portal) -> Self {
        Id {
            hash,
            portal,
            _marker: PhantomData,
        }
    }

    pub fn reify(&self) -> Result<C, Error>
    where
        C: Archive,
        C::Archived: Check<C>,
    {
        self.portal.get(self)
    }

    pub fn hash(&self) -> &IdHash {
        &self.hash
    }
}
