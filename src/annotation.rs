use core::borrow::Borrow;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

use canonical::{Canon, Repr, Store, ValMut};
use canonical_derive::Canon;

use crate::compound::Compound;

pub trait Annotation<L>
where
    Self: Sized,
{
    fn identity() -> Self;
    fn from_leaf(leaf: &L) -> Self;
    fn op(self, b: &Self) -> Self;
}

pub struct AnnMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    annotation: &'a mut C::Annotation,
    compound: ValMut<'a, C>,
    _marker: PhantomData<S>,
}

impl<'a, C, S> Deref for AnnMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.compound
    }
}

impl<'a, C, S> DerefMut for AnnMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.compound
    }
}

impl<'a, C, S> Drop for AnnMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    fn drop(&mut self) {
        *self.annotation = self.compound.annotation()
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
    pub fn new(compound: C) -> Result<Self, S::Error> {
        let a = compound.annotation();
        Ok(Annotated(Repr::<C, S>::new(compound)?, a))
    }

    pub fn annotation(&self) -> &C::Annotation {
        &self.1
    }

    pub fn val_mut(&mut self) -> Result<AnnMut<C, S>, S::Error> {
        Ok(AnnMut {
            annotation: &mut self.1,
            compound: self.0.val_mut()?,
            _marker: PhantomData,
        })
    }
}

impl<C, S> Deref for Annotated<C, S>
where
    C: Compound<S>,
    S: Store,
{
    type Target = Repr<C, S>;

    fn deref(&self) -> &Self::Target {
        &self.0
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

impl<L> Annotation<L> for Cardinality {
    fn identity() -> Self {
        Cardinality(0)
    }

    fn from_leaf(_: &L) -> Self {
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

impl<K, L> Annotation<L> for Max<K>
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
    use canonical_host::MemStore;

    use crate::compound::{Child, Nth};

    #[derive(Clone, Canon)]
    struct Recepticle<T>(Vec<T>);

    impl<T, S> Compound<S> for Recepticle<T>
    where
        T: Canon<S>,
        S: Store,
    {
        type Leaf = T;
        type Annotation = Cardinality;

        fn child(&self, ofs: usize) -> Child<Self, S> {
            match self.0.get(ofs) {
                Some(l) => Child::Leaf(l),
                None => Child::EndOfNode,
            }
        }
    }

    #[test]
    fn annotated() -> Result<(), <MemStore as Store>::Error> {
        let mut hello = Annotated::<_, MemStore>::new(Recepticle(vec![]))?;

        assert_eq!(hello.annotation(), &Cardinality(0));

        hello.val_mut()?.0.push(0u64);

        assert_eq!(hello.annotation(), &Cardinality(1));

        hello.val_mut()?.0.push(0u64);

        assert_eq!(hello.annotation(), &Cardinality(2));

        Ok(())
    }

    #[test]
    fn nth() -> Result<(), <MemStore as Store>::Error> {
        let mut hello = Annotated::<_, MemStore>::new(Recepticle(vec![]))?;

        let n: u64 = 16;

        for i in 0..n {
            hello.val_mut()?.0.push(i);
        }

        for i in 0..n {
            assert_eq!(*Nth::<MemStore>::nth(&*hello.val()?, i)?.unwrap(), i)
        }

        Ok(())
    }
}
