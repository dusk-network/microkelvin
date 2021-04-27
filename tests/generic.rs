// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use microkelvin::{GenericChild, GenericTree};

use canonical_fuzz::fuzz_canon;

#[test]
fn fuzz_generic_tree() {
    fuzz_canon::<GenericTree>()
}

#[test]
fn fuzz_generic_child() {
    fuzz_canon::<GenericChild>()
}
