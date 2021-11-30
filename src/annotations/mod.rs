// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::cell::Ref;
use core::ops::Deref;

use rkyv::Archive;

use crate::{Compound, Primitive, Store};

mod cardinality;
mod max_key;
mod unit;

// re-exports
pub use cardinality::{Cardinality, Nth};
pub use max_key::{FindMaxKey, Keyed, MaxKey, Member};

/// The trait defining an annotation type over a leaf
pub trait Annotation<Leaf>:
    Default + Clone + Combine<Self> + Primitive
{
    /// Creates an annotation from the leaf type
    fn from_leaf(leaf: &Leaf) -> Self;

    /// Create an annotation from a node
    fn from_node<C, S>(node: &C) -> Self
    where
        S: Store,
        C: Compound<Self, S, Leaf = Leaf>,
        C::Leaf: Archive,
    {
        let mut a = Self::default();
        for i in 0.. {
            match node.child(i) {
                crate::Child::Leaf(leaf) => a.combine(&Self::from_leaf(leaf)),
                crate::Child::Link(link) => a.combine(&*link.annotation()),
                crate::Child::Empty => (),
                crate::Child::End => return a,
            }
        }
        unreachable!()
    }
}

/// Trait for defining how to combine Annotations
pub trait Combine<A> {
    /// Combines multiple annotations
    fn combine(&mut self, with: &A);
}

/// A wrapped annotation that is either owning its A or providing an annotated
/// link
#[derive(Debug)]
pub enum ARef<'a, A> {
    /// The annotation is owned
    Owned(A),
    /// The annotation is a reference
    Borrowed(&'a A),
    /// Referenced
    Referenced(Ref<'a, Option<A>>),
}

impl<'a, A> Deref for ARef<'a, A> {
    type Target = A;

    fn deref(&self) -> &Self::Target {
        match self {
            ARef::Owned(ref a) => a,
            ARef::Borrowed(a) => *a,
            ARef::Referenced(r) => &r.as_ref().unwrap(),
        }
    }
}
