// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use rkyv::{ser::Serializer, Archive, Serialize};

use super::Stored;

use super::Portal;

/// A representation of the current storage state
pub struct Storage;

impl Serializer for Storage {
    fn pos(&self) -> usize {
        todo!()
    }

    fn write(&mut self, _bytes: &[u8]) -> Result<(), Self::Error> {
        todo!();
    }
}

impl Portal {
    /// Gets a value from the portal at offset `ofs`
    pub fn get<'a, T>(_ofs: Stored<T>) -> &'a T::Archived
    where
        T: Archive,
    {
        todo!()
    }

    /// Commits a value to the portal
    pub fn put<T>(_t: &T) -> Stored<T>
    where
        T: Archive + Serialize<Storage>,
    {
        todo!()
    }
}

impl Storage {
    /// Commits a value to storage    
    pub fn put<T>(&mut self, _t: &T) -> Stored<T>
    where
        T: Archive + Serialize<Storage>,
    {
        todo!()
    }
}
