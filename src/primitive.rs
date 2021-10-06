// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use rkyv::Archive;

/// Marker trait for types that have themselves as archived type
pub trait Primitive: Archive<Archived = Self> {}

impl<T> Primitive for T where T: Archive<Archived = T> {}
