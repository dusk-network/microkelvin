// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::ops::{Deref, DerefMut};

use canonical::{Canon, CanonError, Repr, ValMut};
use canonical_derive::Canon;

use crate::compound::Compound;

use alloc::rc::Rc;

mod cardinality;
mod max;
mod unit;

/// Custom smart pointer, like a lightweight `std::borrow::Cow` since it
/// is not available in `core`
pub enum Ann<'a, T> {
    Owned(T),
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
pub use max::Max;

pub trait Annotation<Leaf>: Default + Clone {
    fn from_leaf(leaf: &Leaf) -> Self;
    fn combine(annotations: &[Ann<Self>]) -> Self;
}

pub struct AnnRefMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf> + Clone,
{
    annotation: &'a mut A,
    compound: ValMut<'a, C>,
}

impl<'a, C, A> Deref for AnnRefMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf> + Clone,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &*self.compound
    }
}

impl<'a, C, A> DerefMut for AnnRefMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf> + Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.compound
    }
}

impl<'a, C, A> Drop for AnnRefMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf> + Clone,
{
    fn drop(&mut self) {
        *self.annotation = C::annotate_node(&*self.compound)
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
    A: Annotation<C::Leaf> + Clone,
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
    pub fn val(&self) -> Result<Rc<C>, CanonError> {
        self.0.val()
    }

    /// Returns a Mutable annotated reference to the underlying type
    pub fn val_mut(&mut self) -> Result<AnnRefMut<C, A>, CanonError>
    where
        A: Clone,
    {
        Ok(AnnRefMut {
            annotation: Rc::make_mut(&mut self.1),
            compound: self.0.val_mut()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use alloc::vec::Vec;
    use core::marker::PhantomData;

    use crate::annotations::Nth;
    use crate::compound::{Child, ChildMut};
    use canonical::Canon;
    use canonical_derive::Canon;

    #[derive(Clone, Canon)]
    struct Recepticle<T, A>(Vec<T>, PhantomData<A>);

    impl<T, A> Compound<A> for Recepticle<T, A> {
        type Leaf = T;

        fn child(&self, ofs: usize) -> Child<Self, A> {
            match self.0.get(ofs) {
                Some(leaf) => Child::Leaf(leaf),
                None => Child::EndOfNode,
            }
        }

        /// Returns a mutable reference to a possible child at specified offset
        fn child_mut(&mut self, ofs: usize) -> ChildMut<Self, A> {
            match self.0.get_mut(ofs) {
                Some(leaf) => ChildMut::Leaf(leaf),
                None => ChildMut::EndOfNode,
            }
        }
    }

    impl<T> Recepticle<T, Cardinality>
    where
        T: Canon,
    {
        fn new() -> Self {
            Recepticle(Vec::new(), PhantomData)
        }

        fn push(&mut self, t: T) {
            self.0.push(t)
        }

        fn get(&self, i: usize) -> Option<&T> {
            self.0.get(i)
        }

        fn get_mut(&mut self, i: usize) -> Option<&mut T> {
            self.0.get_mut(i)
        }
    }

    #[test]
    fn nth() -> Result<(), CanonError> {
        const N: usize = 16;
        let n = N as u64;

        let mut hello: Recepticle<u64, Cardinality> = Recepticle::new();

        for i in 0..n {
            hello.push(i);
        }

        for i in 0..n {
            assert_eq!(*hello.nth(i)?.unwrap(), i)
        }

        Ok(())
    }

    #[test]
    fn nth_mut() -> Result<(), CanonError> {
        const N: usize = 16;
        let n = N as u64;

        let mut hello: Recepticle<_, Cardinality> = Recepticle::new();

        for i in 0..n {
            hello.push(i);
        }

        for i in 0..n {
            *hello.nth_mut(i)?.expect("Some") += 1;
        }

        for i in 0..n {
            assert_eq!(*hello.nth(i)?.unwrap(), i + 1)
        }

        Ok(())
    }

    // #[test]
    // fn ordering() {
    //     const N_INF: Max<i32> = Max::NegativeInfinity;

    //     assert!(Max::Maximum(0) > Max::Maximum(-1));
    //     assert!(Max::Maximum(-1234) > Max::NegativeInfinity);
    //     assert!(N_INF < Max::Maximum(-1234));
    // }
}
