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

#[derive(Debug)]
/// Custom smart pointer, like a lightweight `std::borrow::Cow` since it
/// is not available in `core`
pub enum Ann<'a, T> {
    /// The annotation is owned
    Owned(T),
    /// The annotation is a reference
    Borrowed(&'a T),
}

impl<'a, T> Deref for Ann<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        match self {
            Ann::Owned(ref t) => t,
            Ann::Borrowed(t) => t,
        }
    }
}

// re-exports
pub use cardinality::{Cardinality, Nth};
pub use max_key::{GetMaxKey, Keyed, MaxKey};

/// The trait defining an annotation type over a leaf
pub trait Annotation<Leaf>: Default + Clone {
    /// Creates an annotation from the leaf type
    fn from_leaf(leaf: &Leaf) -> Self;
    /// Combines multiple annotations in an associative way
    fn combine(annotations: &[Ann<Self>]) -> Self;
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
    A: Annotation<C::Leaf>,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &*self.val
    }
}

#[derive(Debug)]
/// Smart pointer that automatically updates its annotation on drop
pub struct AnnRefMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    annotation: &'a mut A,
    val: ValMut<'a, C>,
}

impl<'a, C, A> AnnRefMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    pub fn annotation(&self) -> &A {
        self.annotation
    }
}

impl<'a, C, A> Deref for AnnRefMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &*self.val
    }
}

impl<'a, C, A> DerefMut for AnnRefMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.val
    }
}

impl<'a, C, A> Drop for AnnRefMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    fn drop(&mut self) {
        *self.annotation = C::annotate_node(&*self.val)
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
    pub fn new(compound: C) -> Self {
        let a = C::annotate_node(&compound);
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
    pub fn val_mut(&mut self) -> Result<AnnRefMut<C, A>, CanonError> {
        Ok(AnnRefMut {
            annotation: Rc::make_mut(&mut self.1),
            val: self.0.val_mut()?,
        })
    }
}
