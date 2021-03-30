// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use crate::annotations::Annotation;
use crate::compound::Compound;

/// The argument given to a closure to `walk` a `Branch`.
pub enum Walk<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    /// Walk encountered a leaf
    Leaf(&'a C::Leaf),
    /// Walk encountered an annotated node
    Ann(&'a A),
}

/// The return value from a closure to `walk` the tree.
///
/// Determines how the `Branch` is constructed
pub enum Step {
    /// The correct leaf was found!
    Found,
    /// Step to the next child on this level
    Next,
    /// Traverse the branch deeper
    Into,
    /// Abort search
    Abort,
}
