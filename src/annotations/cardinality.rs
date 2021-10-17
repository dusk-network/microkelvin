// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

/// Annotation to keep track of the cardinality,
/// i.e. the amount of elements in a collection
use core::borrow::Borrow;

use rend::LittleEndian;
use rkyv::{Archive, Deserialize, Serialize};

use crate::annotations::{Annotation, Combine};
use crate::branch::Branch;
use crate::branch_mut::BranchMut;
use crate::compound::{AnnoIter, ArchivedCompound, Compound, MutableLeaves};
use crate::walk::{Slot, Slots, Step, Walker};

/// The cardinality of a compound collection
#[derive(
    PartialEq, Debug, Clone, Default, Copy, Archive, Serialize, Deserialize,
)]
#[archive(as = "Self")]
pub struct Cardinality(pub(crate) LittleEndian<u64>);

impl From<Cardinality> for u64 {
    fn from(c: Cardinality) -> Self {
        c.0.into()
    }
}

impl<'a> From<&'a Cardinality> for u64 {
    fn from(c: &'a Cardinality) -> Self {
        c.0.into()
    }
}

impl<L> Annotation<L> for Cardinality {
    fn from_leaf(_: &L) -> Self {
        Cardinality(1.into())
    }
}

impl<A> Combine<A> for Cardinality
where
    A: Borrow<Self>,
{
    fn combine<C>(iter: AnnoIter<C, A>) -> Self
    where
        C: Archive + Compound<A>,
        C::Archived: ArchivedCompound<C, A>,
        A: Annotation<C::Leaf>,
    {
        Cardinality(iter.fold(LittleEndian::from(0), |sum, ann| {
            let add: LittleEndian<_> = (*ann).borrow().0;
            (sum + add).into()
        }))
    }
}

/// Walker method to find the nth element of a compound collection
#[derive(Debug)]
pub struct Offset(LittleEndian<u64>);

impl<C, A> Walker<C, A> for Offset
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf> + Borrow<Cardinality>,
{
    fn walk(&mut self, walk: impl Slots<C, A>) -> Step {
        for i in 0.. {
            match walk.slot(i) {
                Slot::Leaf(_) => {
                    if self.0 == 0 {
                        return Step::Found(i);
                    } else {
                        self.0 -= 1;
                    }
                }
                Slot::ArchivedLeaf(_) => {
                    if self.0 == 0 {
                        return Step::Found(i);
                    } else {
                        self.0 -= 1;
                    }
                }
                Slot::Annotation(a) => {
                    let card: &Cardinality = (*a).borrow();
                    if card.0 <= self.0 {
                        self.0 -= card.0;
                    } else {
                        return Step::Found(i);
                    }
                }
                Slot::Empty => (),
                Slot::End => return Step::Abort,
            };
        }
        unreachable!()
    }
}

/// Trait that provides `nth()` and `nth_mut()` methods to any Compound with a
/// Cardinality annotation
pub trait Nth<'a, A>
where
    Self: Archive + Compound<A>,
    Self::Archived: ArchivedCompound<Self, A>,
    A: Annotation<Self::Leaf>,
{
    /// Construct a `Branch` pointing to the `nth` element, if any
    fn nth<N: Into<LittleEndian<u64>>>(
        &'a self,
        n: N,
    ) -> Option<Branch<'a, Self, A>>;

    /// Construct a `BranchMut` pointing to the `nth` element, if any
    fn nth_mut<N: Into<LittleEndian<u64>>>(
        &'a mut self,
        n: N,
    ) -> Option<BranchMut<'a, Self, A>>
    where
        Self: MutableLeaves + Clone;
}

impl<'a, C, A> Nth<'a, A> for C
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf> + Borrow<Cardinality>,
{
    fn nth<N: Into<LittleEndian<u64>>>(
        &'a self,
        ofs: N,
    ) -> Option<Branch<'a, C, A>> {
        // Return the first that satisfies the walk
        Branch::<_, A>::walk(self, Offset(ofs.into()))
    }

    fn nth_mut<N: Into<LittleEndian<u64>>>(
        &'a mut self,
        ofs: N,
    ) -> Option<BranchMut<'a, C, A>>
    where
        C: MutableLeaves + Clone,
    {
        // Return the first mutable branch that satisfies the walk
        BranchMut::<_, A>::walk(self, Offset(ofs.into()))
    }
}
