use core::ops::{Deref, DerefMut};

use canonical::{Canon, Repr, Store, ValMut};
use canonical_derive::Canon;

use crate::compound::Compound;

pub trait Annotation<L>: Clone {
    fn identity() -> Self;
    fn from_leaf(leaf: &L) -> Self;
    fn op(a: &Self, b: &Self) -> Self;
}

pub struct AnnMut<'a, C, A>
where
    C: Compound<Annotation = A>,
    A: Annotation<C::Leaf>,
{
    annotation: &'a mut A,
    compound: ValMut<'a, C>,
}

impl<'a, C, A> Deref for AnnMut<'a, C, A>
where
    C: Compound<Annotation = A>,
    A: Annotation<C::Leaf>,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.compound
    }
}

impl<'a, C, A> DerefMut for AnnMut<'a, C, A>
where
    C: Compound<Annotation = A>,
    A: Annotation<C::Leaf>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.compound
    }
}

impl<'a, C, A> Drop for AnnMut<'a, C, A>
where
    C: Compound<Annotation = A>,
    A: Annotation<C::Leaf>,
{
    fn drop(&mut self) {
        *self.annotation = self.compound.annotation()
    }
}

#[derive(Clone, Canon, Debug)]
pub struct Annotated<C, A, S: Store>(Repr<C, S>, A);

impl<C, A, S> Annotated<C, A, S>
where
    C: Canon<S> + Compound<Annotation = A>,
    A: Annotation<C::Leaf>,
    S: Store,
{
    pub fn new(compound: C) -> Result<Self, S::Error> {
        let a: A = compound.annotation();
        Ok(Annotated(Repr::<C, S>::new(compound)?, a))
    }

    pub fn annotation(&self) -> &A {
        &self.1
    }

    pub fn val_mut(&mut self) -> Result<AnnMut<C, A>, S::Error> {
        Ok(AnnMut {
            annotation: &mut self.1,
            compound: self.0.val_mut()?,
        })
    }
}

impl<T, A, S: Store> Deref for Annotated<T, A, S> {
    type Target = Repr<T, S>;

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

    fn op(a: &Self, b: &Self) -> Self {
        Cardinality(a.0 + b.0)
    }
}

#[derive(Canon, PartialEq, Debug, Clone, Copy)]
pub enum Max {
    NegativeInfinity,
    Maximum(u64),
}

impl Annotation<u64> for Max {
    fn identity() -> Self {
        Max::NegativeInfinity
    }

    fn from_leaf(leaf: &u64) -> Self {
        Max::Maximum(*leaf)
    }

    fn op(a: &Self, b: &Self) -> Self {
        match (a, b) {
            (a, Max::NegativeInfinity) => *a,
            (Max::NegativeInfinity, b) => *b,
            (Max::Maximum(a), Max::Maximum(b)) => {
                if a > b {
                    Max::Maximum(*a)
                } else {
                    Max::Maximum(*b)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::compound::Traverse;
    use canonical_host::MemStore;

    #[test]
    fn annotated() -> Result<(), <MemStore as Store>::Error> {
        #[derive(Clone, Canon)]
        struct Recepticle<T>(Vec<T>);

        impl<T> Compound for Recepticle<T> {
            type Leaf = T;
            type Annotation = Cardinality;

            fn annotation(&self) -> Self::Annotation {
                self.0.iter().fold(Annotation::<T>::identity(), |a, t| {
                    Annotation::<T>::op(&a, &Cardinality::from_leaf(t))
                })
            }

            fn traverse<M: Annotation<<Self as Compound>::Leaf>>(
                &self,
                method: &mut M,
            ) -> Traverse {
                todo!()
            }
        }

        let mut hello =
            Annotated::<_, Cardinality, MemStore>::new(Recepticle(vec![]))?;

        assert_eq!(hello.annotation(), &Cardinality(0));

        hello.val_mut()?.0.push(0u64);

        assert_eq!(hello.annotation(), &Cardinality(1));

        hello.val_mut()?.0.push(0u64);

        assert_eq!(hello.annotation(), &Cardinality(2));

        Ok(())
    }
}
