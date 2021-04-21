// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::ops::{Deref, DerefMut};

use canonical::{CanonError, Repr, Val, ValMut};
use canonical_derive::Canon;

use crate::compound::Compound;

use alloc::rc::Rc;

mod cardinality;
mod max_key;
mod unit;

// re-exports
pub use cardinality::{Cardinality, Nth};
pub use max_key::{GetMaxKey, Keyed, MaxKey};

/// The trait defining an annotation type over a leaf
pub trait Annotation<Leaf>: Default + Clone {
    /// Creates an annotation from the leaf type
    fn from_leaf(leaf: &Leaf) -> Self;
}

/// Trait for defining how to combine Annotations
pub trait Combine<C, A>: Annotation<C::Leaf>
where
    C: Compound<A>,
{
    /// Combines multiple annotations
    fn combine(node: &C) -> Self;
}

#[derive(Debug)]
/// Reference to an annotated value, along with it annotation
pub struct AnnRef<'a, C, A> {
    annotation: &'a A,
    val: Val<'a, C>,
}

impl<'a, C, A> Deref for AnnRef<'a, C, A>
where
    C: Compound<A>,
    A: Combine<C, A>,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &*self.val
    }
}

#[derive(Debug)]
/// Custom pointer type, like a lightweight `std::borrow::Cow` since it
/// is not available in `core`
pub enum WrappedAnnotation<'a, A> {
    /// The annotation is owned
    Owned(A),
    /// The annotation is a reference
    Borrowed(&'a A),
}

impl<'a, A> Deref for WrappedAnnotation<'a, A> {
    type Target = A;

    fn deref(&self) -> &Self::Target {
        match self {
            WrappedAnnotation::Owned(ref a) => a,
            WrappedAnnotation::Borrowed(a) => a,
        }
    }
}

#[derive(Debug)]
/// Smart pointer that automatically updates its annotation on drop
pub struct AnnRefMut<'a, C, A>
where
    C: Compound<A>,
    A: Combine<C, A>,
{
    annotation: &'a mut A,
    val: ValMut<'a, C>,
}

impl<'a, C, A> AnnRefMut<'a, C, A>
where
    C: Compound<A>,
    A: Combine<C, A>,
{
    pub fn annotation(&self) -> &A {
        self.annotation
    }
}

impl<'a, C, A> Deref for AnnRefMut<'a, C, A>
where
    C: Compound<A>,
    A: Combine<C, A>,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &*self.val
    }
}

impl<'a, C, A> DerefMut for AnnRefMut<'a, C, A>
where
    C: Compound<A>,
    A: Combine<C, A>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.val
    }
}

impl<'a, C, A> Drop for AnnRefMut<'a, C, A>
where
    C: Compound<A>,
    A: Combine<C, A>,
{
    fn drop(&mut self) {
        *self.annotation = A::combine(&*self.val)
    }
}

#[derive(Debug, Canon)]
/// A wrapper type that keeps the annotation of the Compound referenced cached
pub struct Annotated<C, A>(Repr<C>, Rc<A>);

impl<C, A> Clone for Annotated<C, A> {
    fn clone(&self) -> Self {
        Annotated(self.0.clone(), self.1.clone())
    }
}

impl<C, A> Annotated<C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    /// Create a new annotated type
    pub fn new(compound: C) -> Self
    where
        A: Combine<C, A>,
    {
        let a = A::combine(&compound);
        Annotated(Repr::new(compound), Rc::new(a))
    }

    /// Returns a reference to to the annotation stored
    pub fn annotation(&self) -> &A {
        &self.1
    }

    /// Returns an annotated reference to the underlying type
    pub fn val(&self) -> Result<AnnRef<C, A>, CanonError> {
        Ok(AnnRef {
            val: self.0.val()?,
            annotation: &self.1,
        })
    }

    /// Returns a Mutable annotated reference to the underlying type
    pub fn val_mut(&mut self) -> Result<AnnRefMut<C, A>, CanonError>
    where
        A: Combine<C, A>,
    {
        Ok(AnnRefMut {
            annotation: Rc::make_mut(&mut self.1),
            val: self.0.val_mut()?,
        })
    }
}
