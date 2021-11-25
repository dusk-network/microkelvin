// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

#![allow(clippy::unused_unit)]

use crate::annotations::{Annotation, Combine};

impl<L> Annotation<L> for () {
    fn from_leaf(_: &L) -> Self {
        ()
    }
}

impl<A> Combine<A> for () {
    fn combine(&mut self, _: &A) -> Self {
        ()
    }
}
