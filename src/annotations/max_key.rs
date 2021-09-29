// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

/// Annotation to keep track of the largest element of a collection
use core::borrow::Borrow;
use core::cmp::Ordering;
use core::marker::PhantomData;

use rkyv::{Archive, Serialize};

use crate::annotations::{Annotation, Combine};
use crate::branch::Branch;
use crate::branch_mut::BranchMut;
use crate::compound::{AnnoIter, ArchivedChildren, Compound, MutableLeaves};
use crate::primitive::Primitive;
use crate::walk::{Slots, Step, Walker};

/// The maximum value of a collection
#[derive(PartialEq, Eq, Clone, Debug, Archive, Serialize)]
#[archive(as = "Self")]
#[archive(bound(archive = "K: Primitive"))]
pub enum MaxKey<K> {
    /// Identity of max, everything else is larger
    NegativeInfinity,
    /// Actual max value
    Maximum(K),
}

/// Trait for getting the key from a Leaf value
pub trait Keyed<K> {
    /// Return a reference to the key of the leaf type
    fn key(&self) -> &K;
}

// Elements can be their own keys
impl<T> Keyed<T> for T {
    fn key(&self) -> &T {
        self
    }
}

impl<K> Default for MaxKey<K> {
    fn default() -> Self {
        MaxKey::NegativeInfinity
    }
}

impl<K> PartialOrd for MaxKey<K>
where
    K: PartialOrd,
{
    fn partial_cmp(&self, other: &MaxKey<K>) -> Option<Ordering> {
        match (self, other) {
            (MaxKey::NegativeInfinity, MaxKey::NegativeInfinity) => {
                Some(Ordering::Equal)
            }
            (_, MaxKey::NegativeInfinity) => Some(Ordering::Greater),
            (MaxKey::NegativeInfinity, _) => Some(Ordering::Less),
            (MaxKey::Maximum(a), MaxKey::Maximum(b)) => a.partial_cmp(b),
        }
    }
}

impl<K> Ord for MaxKey<K>
where
    K: Ord,
{
    fn cmp(&self, other: &MaxKey<K>) -> Ordering {
        match (self, other) {
            (MaxKey::NegativeInfinity, MaxKey::NegativeInfinity) => {
                Ordering::Equal
            }
            (_, MaxKey::NegativeInfinity) => Ordering::Greater,
            (MaxKey::NegativeInfinity, _) => Ordering::Less,
            (MaxKey::Maximum(a), MaxKey::Maximum(b)) => a.cmp(b),
        }
    }
}

impl<K, L> Annotation<L> for MaxKey<K>
where
    L: Keyed<K>,
    K: Primitive + Clone + Ord,
{
    fn from_leaf(leaf: &L) -> Self {
        MaxKey::Maximum(leaf.key().clone())
    }
}

impl<K, A> Combine<A> for MaxKey<K>
where
    K: Ord + Clone,
    A: Primitive + Borrow<Self>,
{
    fn combine<C>(iter: AnnoIter<C, A>) -> Self
    where
        C: Compound<A>,
        A: Annotation<C::Leaf>,
    {
        iter.fold(MaxKey::NegativeInfinity, |max, ann| {
            let m = (*ann).borrow();
            // We only clone if neccesary
            if *m > max {
                m.clone()
            } else {
                max
            }
        })
    }
}

/// Walker to find the maximum key in the collection
pub struct FindMaxKey<K>(PhantomData<K>);

impl<K> Default for FindMaxKey<K> {
    fn default() -> Self {
        FindMaxKey(PhantomData)
    }
}

impl<C, A, K> Walker<C, A> for FindMaxKey<K>
where
    C: Compound<A>,
    C::Archived: ArchivedChildren<C, A>,
    C::Leaf: Keyed<K>,
    A: Primitive + Annotation<C::Leaf> + Borrow<MaxKey<K>>,
    K: Ord + Clone,
{
    fn walk(&mut self, _walk: impl Slots<C, A>) -> Step {
        // let mut current_max: MaxKey<K> = MaxKey::NegativeInfinity;
        // let mut current_step = Step::Abort;

        // for i in 0.. {
        //     match walk.child(i) {
        //         WalkChild::Leaf(l) => {
        //             let leaf_max: MaxKey<K> =
        // MaxKey::Maximum(l.key().clone());

        //             if leaf_max > current_max {
        //                 current_max = leaf_max;
        //                 current_step = Step::Found(i);
        //             }
        //         }
        //         WalkChild::Annotation(ann) => {
        //             let node_max: &MaxKey<K> = (*ann).borrow();
        //             if node_max > &current_max {
        //                 current_max = node_max.clone();
        //                 current_step = Step::Into(i);
        //             }
        //         }
        //         WalkChild::Empty => (),
        //         WalkChild::EndOfNode => return current_step,
        //     }
        // }
        // unreachable!()
        todo!()
    }
}

/// Trait that provides a max_leaf() method to any Compound with a MaxKey
/// annotation
pub trait GetMaxKey<'a, A, K>
where
    Self: Compound<A>,
    Self::Leaf: Keyed<K> + Archive,
    Self::Archived: ArchivedChildren<Self, A>,
    A: Primitive + Annotation<Self::Leaf> + Borrow<MaxKey<K>>,
    K: Ord,
{
    /// Construct a `Branch` pointing to the element with the largest key
    fn max_key(&'a self) -> Option<Branch<'a, Self, A>>;

    /// Construct a `BranchMut` pointing to the element with the largest key
    fn max_key_mut(&'a mut self) -> Option<BranchMut<'a, Self, A>>
    where
        Self: MutableLeaves + Clone;
}

impl<'a, C, A, K> GetMaxKey<'a, A, K> for C
where
    C: Compound<A>,
    C::Leaf: Keyed<K>,
    C::Archived: ArchivedChildren<C, A>,
    A: Primitive + Annotation<C::Leaf> + Borrow<MaxKey<K>>,
    K: Ord + Clone,
{
    fn max_key(&'a self) -> Option<Branch<'a, Self, A>> {
        // Return the first that satisfies the walk
        Branch::<_, A>::walk(self, FindMaxKey::default())
    }

    fn max_key_mut(&'a mut self) -> Option<BranchMut<'a, Self, A>>
    where
        C: MutableLeaves + Clone,
    {
        // Return the first mutable branch that satisfies the walk
        BranchMut::<_, A>::walk(self, FindMaxKey::default())
    }
}
