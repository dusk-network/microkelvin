// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::cell::Ref;
use core::ops::Deref;

use owning_ref::OwningRef;

use crate::{AnnoIter, ArchivedChildren, Compound, Primitive};

mod cardinality;
mod max_key;
mod unit;

// re-exports
pub use cardinality::{Cardinality, Nth};
pub use max_key::{GetMaxKey, Keyed, MaxKey};

/// The trait defining an annotation type over a leaf
pub trait Annotation<Leaf>: Default + Clone + Combine<Self> {
    /// Creates an annotation from the leaf type
    fn from_leaf(leaf: &Leaf) -> Self;
}

// TODO- move C into the trait, not the method.

/// Trait for defining how to combine Annotations
pub trait Combine<A> {
    /// Combines multiple annotations
    fn combine<C>(iter: AnnoIter<C, A>) -> Self
    where
        C: Compound<A>,
        C::Archived: ArchivedChildren<C, A>,
        A: Primitive + Annotation<C::Leaf>;
}

/// A wrapped annotation that is either owning it's a or providing an annotated
/// link
#[derive(Debug)]
pub enum ARef<'a, A> {
    /// The annotation is owned
    Owned(A),
    /// The annotation is a reference
    Borrowed(&'a A),
    /// Referenced
    Referenced(OwningRef<Ref<'a, Option<A>>, A>),
}

impl<'a, A> Deref for ARef<'a, A> {
    type Target = A;

    fn deref(&self) -> &Self::Target {
        match self {
            ARef::Owned(ref a) => a,
            ARef::Borrowed(a) => *a,
            ARef::Referenced(a) => a,
        }
    }
}
