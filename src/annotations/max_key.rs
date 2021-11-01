// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

/// Annotation to keep track of the largest element of a collection
use core::borrow::Borrow;
use core::cmp::Ordering;
use core::marker::PhantomData;

use rkyv::{Archive, Deserialize, Serialize};

use crate::annotations::{Annotation, Combine};
use crate::compound::{AnnoIter, Compound};
use crate::walk::{Slot, Slots, Step, Walker};
use crate::wrappers::Primitive;
use crate::ArchivedCompound;

/// The maximum value of a collection
#[derive(PartialEq, Eq, Clone, Debug, Archive, Serialize, Deserialize)]
#[archive(as = "Self")]
#[archive(bound(archive = "
  K: Primitive"))]
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
        C: Archive + Compound<A>,
        C::Archived: ArchivedCompound<C, A>,
        C::Leaf: Archive,
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
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    C::Leaf: Archive + Keyed<K>,
    <C::Leaf as Archive>::Archived: Keyed<K>,
    A: Annotation<C::Leaf> + Borrow<MaxKey<K>>,
    K: Ord + Clone,
{
    fn walk(&mut self, walk: impl Slots<C, A>) -> Step {
        let mut current_max: MaxKey<K> = MaxKey::NegativeInfinity;
        let mut current_step = Step::Abort;

        for i in 0.. {
            match walk.slot(i) {
                Slot::Leaf(l) => {
                    let leaf_max: MaxKey<K> = MaxKey::Maximum(l.key().clone());

                    if leaf_max > current_max {
                        current_max = leaf_max;
                        current_step = Step::Found(i);
                    }
                }
                Slot::ArchivedLeaf(l) => {
                    let leaf_max: MaxKey<K> = MaxKey::Maximum(l.key().clone());

                    if leaf_max > current_max {
                        current_max = leaf_max;
                        current_step = Step::Found(i);
                    }
                }
                Slot::Annotation(ann) => {
                    let node_max: &MaxKey<K> = (*ann).borrow();
                    if node_max > &current_max {
                        current_max = node_max.clone();
                        current_step = Step::Found(i);
                    }
                }
                Slot::Empty => (),
                Slot::End => return current_step,
            }
        }
        unreachable!()
    }
}
