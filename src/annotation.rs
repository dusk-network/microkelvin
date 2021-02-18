// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::borrow::Borrow;
use core::cmp::Ordering;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

use canonical::{Canon, Repr, Sink, Source, Store, Val, ValMut};
use canonical_derive::Canon;

use crate::compound::{Child, Compound};

/// This type can annotate a leaf and a node
pub trait Annotation<N, L, S>
where
    Self: Sized,
{
    /// The identity annotation
    fn identity() -> Self;

    /// Annotate a leaf
    fn from_leaf(leaf: &L) -> Self;

    /// Annotate a node
    fn from_node(node: &N) -> Self;
}

/// A reference o a value carrying an annotation
pub struct AnnRef<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    annotation: &'a C::Annotation,
    compound: Val<'a, C>,
    _marker: PhantomData<S>,
}

impl<'a, C, S> AnnRef<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    pub fn annotation(&self) -> &C::Annotation {
        self.annotation
    }
}

impl<'a, C, S> Deref for AnnRef<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &*self.compound
    }
}

pub struct AnnRefMut<'a, C, S>
where
    C: Compound<S>,
    C::Annotation: Annotation<C, C::Leaf, S>,
    S: Store,
{
    annotation: &'a mut C::Annotation,
    compound: ValMut<'a, C, S>,
    _marker: PhantomData<S>,
}

impl<'a, C, S> Deref for AnnRefMut<'a, C, S>
where
    C: Compound<S>,
    C::Annotation: Annotation<C, C::Leaf, S>,
    S: Store,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.compound
    }
}

impl<'a, C, S> DerefMut for AnnRefMut<'a, C, S>
where
    C: Compound<S>,
    C::Annotation: Annotation<C, C::Leaf, S>,
    S: Store,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.compound
    }
}

impl<'a, C, S> Drop for AnnRefMut<'a, C, S>
where
    C: Compound<S>,
    C::Annotation: Annotation<C, C::Leaf, S>,
    S: Store,
{
    fn drop(&mut self) {
        *self.annotation = C::Annotation::from_node(&*self.compound)
    }
}

#[derive(Clone, Debug)]
/// A wrapper type that keeps the annotation of the Compound referenced cached
pub struct Annotated<C, S>(Repr<C, S>, C::Annotation)
where
    C: Compound<S>,
    S: Store;

// Manual implementation to avoid restraining the type to `Canon` in the trait
// which would be required by the derive macro
impl<C, S> Canon<S> for Annotated<C, S>
where
    C: Compound<S>,
    S: Store,
{
    fn write(&self, sink: &mut impl Sink<S>) -> Result<(), S::Error> {
        self.0.write(sink)?;
        self.1.write(sink)
    }

    fn read(source: &mut impl Source<S>) -> Result<Self, S::Error> {
        Ok(Annotated(Repr::read(source)?, C::Annotation::read(source)?))
    }

    fn encoded_len(&self) -> usize {
        self.0.encoded_len() + self.1.encoded_len()
    }
}

impl<C, S> Annotated<C, S>
where
    C: Compound<S>,
    C::Annotation: Annotation<C, C::Leaf, S>,
    S: Store,
{
    /// Create a new annotated type
    pub fn new(compound: C) -> Self {
        let a = C::Annotation::from_node(&compound);
        Annotated(Repr::<C, S>::new(compound), a)
    }

    /// Returns a reference to to the annotation stored
    pub fn annotation(&self) -> &C::Annotation {
        &self.1
    }

    /// Returns an annotated reference to the underlying type
    pub fn val(&self) -> Result<AnnRef<C, S>, S::Error> {
        Ok(AnnRef {
            annotation: &self.1,
            compound: self.0.val()?,
            _marker: PhantomData,
        })
    }

    /// Returns a Mutable annotated reference to the underlying type
    pub fn val_mut(&mut self) -> Result<AnnRefMut<C, S>, S::Error> {
        Ok(AnnRefMut {
            annotation: &mut self.1,
            compound: self.0.val_mut()?,
            _marker: PhantomData,
        })
    }
}

// implementations

/// Annotation to keep track of the cardinality,
/// i.e. the amount of elements of a collection
#[derive(Canon, PartialEq, Debug, Clone)]
pub struct Cardinality(pub(crate) u64);

impl Into<u64> for &Cardinality {
    fn into(self) -> u64 {
        self.0
    }
}

impl<C, S> Annotation<C, C::Leaf, S> for Cardinality
where
    C: Compound<S>,
    C::Annotation: Annotation<C, C::Leaf, S> + Borrow<Cardinality>,
    S: Store,
{
    fn identity() -> Self {
        Cardinality(0)
    }

    fn from_leaf(_: &C::Leaf) -> Self {
        Cardinality(1)
    }

    fn from_node(node: &C) -> Self {
        let mut c = 0;
        for i in 0.. {
            c += match node.child(i) {
                Child::Leaf(_) => 1,
                Child::Node(n) => n.annotation().borrow().0,
                Child::EndOfNode => return Cardinality(c),
                Child::Empty => 0,
            }
        }
        unreachable!()
    }
}

/// Annotation to keep track of the largest element of a collection
#[derive(Canon, PartialEq, Eq, Debug, Clone, Copy)]
pub enum Max<K> {
    /// Identity of max, everything else is larger
    NegativeInfinity,
    /// Actual max value
    Maximum(K),
}

impl<K> PartialOrd for Max<K>
where
    K: PartialOrd + Eq,
{
    fn partial_cmp(&self, other: &Max<K>) -> Option<Ordering> {
        match (self, other) {
            (Max::NegativeInfinity, Max::NegativeInfinity) => {
                Some(Ordering::Equal)
            }
            (Max::NegativeInfinity, _) => Some(Ordering::Less),
            (_, Max::NegativeInfinity) => Some(Ordering::Greater),
            (Max::Maximum(a), Max::Maximum(b)) => a.partial_cmp(b),
        }
    }
}

impl<K> Ord for Max<K>
where
    K: Ord + Eq,
{
    fn cmp(&self, other: &Max<K>) -> Ordering {
        match (self, other) {
            (Max::NegativeInfinity, Max::NegativeInfinity) => Ordering::Equal,
            (Max::NegativeInfinity, _) => Ordering::Less,
            (_, Max::NegativeInfinity) => Ordering::Greater,
            (Max::Maximum(a), Max::Maximum(b)) => a.cmp(b),
        }
    }
}

impl<K> PartialEq<K> for Max<K>
where
    K: PartialEq,
{
    fn eq(&self, k: &K) -> bool {
        match self {
            Max::NegativeInfinity => false,
            Max::Maximum(k_p) => k_p == k,
        }
    }
}

impl<K> PartialOrd<K> for Max<K>
where
    K: PartialOrd + Eq,
{
    fn partial_cmp(&self, k: &K) -> Option<Ordering> {
        match self {
            Max::NegativeInfinity => Some(Ordering::Less),
            Max::Maximum(k_p) => k_p.partial_cmp(k),
        }
    }
}

impl<N, L, S> Annotation<N, L, S> for () {
    fn identity() -> () {
        ()
    }

    fn from_leaf(_: &L) -> () {
        ()
    }

    fn from_node(_: &N) -> () {
        ()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use alloc::vec::Vec;
    use canonical::Store;
    use canonical_host::MemStore;

    use crate::compound::{Child, ChildMut, Nth};

    #[derive(Clone, Canon)]
    struct Recepticle<T, S>(Vec<T>, PhantomData<S>);

    impl<T, S> Recepticle<T, S>
    where
        T: Canon<S>,
        S: Store,
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
    fn nth() -> Result<(), <MemStore as Store>::Error> {
        const N: usize = 16;
        let n = N as u64;

        let mut hello: Recepticle<u64, Cardinality, MemStore> =
            Recepticle::new();

        for i in 0..n {
            hello.push(i);
        }

        for i in 0..n {
            assert_eq!(*hello.nth(i)?.unwrap(), i)
        }

        Ok(())
    }

    #[test]
    fn nth_mut() -> Result<(), <MemStore as Store>::Error> {
        const N: usize = 16;
        let n = N as u64;

        let mut hello: Recepticle<_, Cardinality, MemStore> = Recepticle::new();

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

    #[test]
    fn ordering() {
        const N_INF: Max<i32> = Max::NegativeInfinity;

        assert!(Max::Maximum(0) > Max::Maximum(-1));
        assert!(Max::Maximum(-1234) > Max::NegativeInfinity);
        assert!(N_INF < Max::Maximum(-1234));
    }
}
