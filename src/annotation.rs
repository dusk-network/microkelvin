// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::borrow::Borrow;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

use canonical::{Canon, Cow, Repr, Store, ValMut};
use canonical_derive::Canon;

use crate::compound::{Child, Compound};

pub trait Annotation<C, S>
where
    C: Compound<S>,
    S: Store,
{
    fn identity() -> Self;
    fn from_leaf(leaf: &C::Leaf) -> Self;
    fn from_node(node: &C) -> Self;
}

pub trait Associative<L> {
    fn identity() -> Self;
    fn from_leaf(leaf: &L) -> Self;
    fn op(self, b: &Self) -> Self;
}

impl<A, C, S> Annotation<C, S> for A
where
    A: Associative<C::Leaf>,
    C: Compound<S, Annotation = A>,
    S: Store,
{
    fn identity() -> Self {
        A::identity()
    }

    fn from_leaf(leaf: &C::Leaf) -> Self {
        A::from_leaf(leaf)
    }

    fn from_node(node: &C) -> Self {
        let mut annotation = Self::identity();
        for i in 0.. {
            match node.child(i) {
                Child::Leaf(l) => {
                    annotation = annotation.op(&Self::from_leaf(l))
                }
                Child::Node(n) => annotation = annotation.op(n.annotation()),
                Child::EndOfNode => return annotation,
            };
        }
        unreachable!()
    }
}

pub struct AnnRef<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    annotation: &'a C::Annotation,
    compound: Cow<'a, C>,
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
    S: Store,
{
    annotation: &'a mut C::Annotation,
    compound: ValMut<'a, C>,
    _marker: PhantomData<S>,
}

impl<'a, C, S> Deref for AnnRefMut<'a, C, S>
where
    C: Compound<S>,
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
    S: Store,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.compound
    }
}

impl<'a, C, S> Drop for AnnRefMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    fn drop(&mut self) {
        *self.annotation = C::Annotation::from_node(&*self.compound)
    }
}

#[derive(Clone, Canon, Debug)]
pub struct Annotated<C, S>(Repr<C, S>, C::Annotation)
where
    C: Compound<S>,
    S: Store;

impl<C, S> Annotated<C, S>
where
    C: Compound<S>,
    C: Canon<S>,
    S: Store,
{
    pub fn new(compound: C) -> Self {
        let a = C::Annotation::from_node(&compound);
        Annotated(Repr::<C, S>::new(compound), a)
    }

    pub fn annotation(&self) -> &C::Annotation {
        &self.1
    }

    pub fn val(&self) -> Result<AnnRef<C, S>, S::Error> {
        Ok(AnnRef {
            annotation: &self.1,
            compound: self.0.val()?,
            _marker: PhantomData,
        })
    }

    pub fn val_mut(&mut self) -> Result<AnnRefMut<C, S>, S::Error> {
        Ok(AnnRefMut {
            annotation: &mut self.1,
            compound: self.0.val_mut()?,
            _marker: PhantomData,
        })
    }
}

// implementations

#[derive(Canon, PartialEq, Debug, Clone)]
pub struct Cardinality(pub(crate) u64);

impl Cardinality {
    pub fn new(i: u64) -> Self {
        Cardinality(i)
    }
}

impl<L> Associative<L> for Cardinality {
    fn identity() -> Self {
        Cardinality(0)
    }

    fn from_leaf(_leaf: &L) -> Self {
        Cardinality(1)
    }

    fn op(mut self, b: &Self) -> Self {
        self.0 += b.0;
        self
    }
}

#[derive(Canon, PartialEq, Debug, Clone, Copy)]
pub enum Max<K> {
    NegativeInfinity,
    Maximum(K),
}

impl<K, L> Associative<L> for Max<K>
where
    K: Ord + Clone,
    L: Borrow<K>,
{
    fn identity() -> Self {
        Max::NegativeInfinity
    }

    fn from_leaf(leaf: &L) -> Self {
        Max::Maximum(leaf.borrow().clone())
    }

    fn op(self, b: &Self) -> Self {
        match (self, b) {
            (a @ Max::Maximum(_), Max::NegativeInfinity) => a,
            (Max::NegativeInfinity, b) => b.clone(),
            (Max::Maximum(ref a), Max::Maximum(b)) => {
                if a > b {
                    Max::Maximum(a.clone())
                } else {
                    Max::Maximum(b.clone())
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use canonical::Store;
    use canonical::{Sink, Source};
    use canonical_host::MemStore;
    use const_arrayvec::ArrayVec;

    use crate::compound::{Child, ChildMut, Nth};

    #[derive(Clone)]
    struct CanonArrayVec<T, const N: usize>(ArrayVec<T, N>);

    impl<T, const N: usize> CanonArrayVec<T, N> {
        pub fn new() -> Self {
            CanonArrayVec(ArrayVec::new())
        }
    }
    impl<S: Store, T: Canon<S>, const N: usize> Canon<S> for CanonArrayVec<T, N> {
        fn write(&self, sink: &mut impl Sink<S>) -> Result<(), S::Error> {
            let len = self.0.len() as u64;
            len.write(sink)?;
            for t in self.0.iter() {
                t.write(sink)?;
            }
            Ok(())
        }

        fn read(source: &mut impl Source<S>) -> Result<Self, S::Error> {
            let mut vec: ArrayVec<T, N> = ArrayVec::new();
            let len = u64::read(source)?;
            for _ in 0..len {
                vec.push(T::read(source)?);
            }
            Ok(CanonArrayVec(vec))
        }

        fn encoded_len(&self) -> usize {
            // length of length
            let mut len = Canon::<S>::encoded_len(&0u64);
            for t in self.0.iter() {
                len += t.encoded_len()
            }
            len
        }
    }

    #[derive(Clone, Canon)]
    struct Recepticle<T, S, const N: usize>(
        CanonArrayVec<T, N>,
        PhantomData<S>,
    );

    impl<T, S, const N: usize> Recepticle<T, S, N>
    where
        T: Canon<S>,
        S: Store,
    {
        fn new() -> Self {
            Recepticle(CanonArrayVec::new(), PhantomData)
        }

        fn push(&mut self, t: T) {
            (self.0).0.push(t)
        }

        fn get(&self, i: usize) -> Option<&T> {
            (self.0).0.get(i)
        }

        fn get_mut(&mut self, i: usize) -> Option<&mut T> {
            (self.0).0.get_mut(i)
        }
    }

    impl<T, S, const N: usize> Compound<S> for Recepticle<T, S, N>
    where
        T: Canon<S>,
        S: Store,
    {
        type Leaf = T;
        type Annotation = Cardinality;

        fn child(&self, ofs: usize) -> Child<Self, S> {
            match self.get(ofs) {
                Some(l) => Child::Leaf(l),
                None => Child::EndOfNode,
            }
        }

        fn child_mut(&mut self, ofs: usize) -> ChildMut<Self, S> {
            match self.get_mut(ofs) {
                Some(l) => ChildMut::Leaf(l),
                None => ChildMut::EndOfNode,
            }
        }
    }

    #[test]
    fn annotated() -> Result<(), <MemStore as Store>::Error> {
        let mut hello: Annotated<Recepticle<u64, MemStore, 4>, MemStore> =
            Annotated::<_, MemStore>::new(Recepticle::new());

        assert_eq!(hello.annotation(), &Cardinality(0));

        hello.val_mut()?.push(0u64);

        assert_eq!(hello.annotation(), &Cardinality(1));

        hello.val_mut()?.push(0u64);

        assert_eq!(hello.annotation(), &Cardinality(2));

        Ok(())
    }

    #[test]
    fn nth() -> Result<(), <MemStore as Store>::Error> {
        const N: usize = 16;
        let n = N as u64;

        let mut hello: Annotated<Recepticle<u64, MemStore, N>, MemStore> =
            Annotated::<_, MemStore>::new(Recepticle::new());

        for i in 0..n {
            hello.val_mut()?.push(i);
        }

        for i in 0..n {
            assert_eq!(*Nth::<MemStore, N>::nth(&*hello.val()?, i)?.unwrap(), i)
        }

        Ok(())
    }

    #[test]
    fn nth_mut() -> Result<(), <MemStore as Store>::Error> {
        const N: usize = 16;
        let n = N as u64;

        let mut hello: Recepticle<_, MemStore, N> = Recepticle::new();

        for i in 0..n {
            hello.push(i);
        }

        for i in 0..n {
            let mut nth: crate::branch_mut::BranchMut<
                '_,
                Recepticle<u64, MemStore, N>,
                MemStore,
                N,
            > = hello.nth_mut(i)?.expect("Some");
            *nth += 1;
        }

        for i in 0..n {
            let nth: crate::branch::Branch<
                '_,
                Recepticle<u64, MemStore, N>,
                MemStore,
                N,
            > = hello.nth(i)?.unwrap();

            assert_eq!(*nth, i + 1)
        }

        Ok(())
    }
}
