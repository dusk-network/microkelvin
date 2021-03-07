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

// re-exports
pub use cardinality::{Cardinality, Nth};
pub use max::Max;

/// The value is an annotation that can be derived from a leaf or a node
// pub trait Annotation<C>
// where
//     C: Compound<A>,
// {
//     /// The identity value of the annotation
//     fn identity() -> Self;
//     /// Compute annotation from node
//     fn from_node(node: &C) -> Self;
//     /// Compute annotation from leaf
//     fn from_leaf(leaf: &C::Leaf) -> Self;
// }

#[derive(Clone)]
/// A reference o a value carrying an annotation
pub struct AnnRef<'a, C, A>
where
    C: Compound<A>,
{
    annotation: &'a A,
    compound: Rc<C>,
}

impl<'a, C, A> AnnRef<'a, C, A>
where
    C: Compound<A>,
{
    pub fn annotation(&self) -> &A {
        self.annotation
    }
}

impl<'a, C, A> Deref for AnnRef<'a, C, A>
where
    C: Compound<A>,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &*self.compound
    }
}

pub struct AnnRefMut<'a, C, A>
where
    C: Compound<A>,
{
    annotation: &'a mut A,
    compound: ValMut<'a, C>,
}

impl<'a, C, A> Deref for AnnRefMut<'a, C, A>
where
    C: Compound<A>,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &*self.compound
    }
}

impl<'a, C, A> DerefMut for AnnRefMut<'a, C, A>
where
    C: Compound<A>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.compound
    }
}

impl<'a, C, A> Drop for AnnRefMut<'a, C, A>
where
    C: Compound<A>,
{
    fn drop(&mut self) {
        *self.annotation = C::annotate_node(&*self.compound)
    }
}

#[derive(Clone, Debug, Canon)]
/// A wrapper type that keeps the annotation of the Compound referenced cached
pub struct Annotated<C, A>(Repr<C>, A)
where
    C: Compound<A>;

impl<C, A> Annotated<C, A>
where
    C: Compound<A>,
{
    /// Create a new annotated type
    pub fn new(compound: C) -> Self {
        let a = C::annotate_node(&compound);
        Annotated(Repr::new(compound), a)
    }

    /// Returns a reference to to the annotation stored
    pub fn annotation(&self) -> &A {
        &self.1
    }

    /// Returns an annotated reference to the underlying type
    pub fn val(&self) -> Result<AnnRef<C, A>, CanonError> {
        Ok(AnnRef {
            annotation: &self.1,
            compound: self.0.val()?,
        })
    }

    /// Returns a Mutable annotated reference to the underlying type
    pub fn val_mut(&mut self) -> Result<AnnRefMut<C, A>, CanonError>
    where
        C: Canon,
    {
        Ok(AnnRefMut {
            annotation: &mut self.1,
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

    impl<T, A> Compound for Recepticle<T, A> {
        type Leaf = T;

        fn child<A2>(&self, ofs: usize) -> Child<Self, A2> {
            match self.0.get(ofs) {
                Some(leaf) => Child::Leaf(leaf),
                None => Child::EndOfNode,
            }
        }

        /// Returns a mutable reference to a possible child at specified offset
        fn child_mut<A2>(&mut self, ofs: usize) -> ChildMut<Self, A2> {
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
