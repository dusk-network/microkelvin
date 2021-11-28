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
use crate::walk::{Discriminant, Step, Walkable, Walker};
use crate::wrappers::Primitive;
use crate::Compound;

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

impl<K> PartialEq<K> for MaxKey<K>
where
    K: PartialEq,
{
    fn eq(&self, other: &K) -> bool {
        match self {
            MaxKey::NegativeInfinity => false,
            MaxKey::Maximum(k) => k == other,
        }
    }
}

impl<K> PartialOrd<K> for MaxKey<K>
where
    K: PartialEq<K>,
    K: PartialOrd,
{
    fn partial_cmp(&self, other: &K) -> Option<Ordering> {
        match self {
            MaxKey::NegativeInfinity => Some(Ordering::Less),
            MaxKey::Maximum(k) => k.partial_cmp(other),
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
    fn combine(&mut self, other: &A) {
        let b = other.borrow();
        if b > self {
            *self = b.clone()
        }
    }
}

/// Walker to find the maximum key in the collection
pub struct FindMaxKey<K>(PhantomData<K>);

impl<K> Default for FindMaxKey<K> {
    fn default() -> Self {
        FindMaxKey(PhantomData)
    }
}

impl<S, C, A, K> Walker<S, C, A> for FindMaxKey<K>
where
    C: Compound<S, A>,
    C::Leaf: Archive + Keyed<K>,
    <C::Leaf as Archive>::Archived: Keyed<K>,
    A: Borrow<MaxKey<K>>,
    K: Ord + Clone,
{
    fn walk(&mut self, walk: impl Walkable<S, C, A>) -> Step {
        let mut current_max: MaxKey<K> = MaxKey::NegativeInfinity;
        let mut current_step = Step::Abort;

        for i in 0.. {
            match walk.probe(i) {
                Discriminant::Leaf(l) => {
                    let leaf_max: MaxKey<K> = MaxKey::Maximum(l.key().clone());

                    if leaf_max > current_max {
                        current_max = leaf_max;
                        current_step = Step::Found(i);
                    }
                }
                Discriminant::Annotation(ann) => {
                    let node_max: &MaxKey<K> = (*ann).borrow();
                    if node_max > &current_max {
                        current_max = node_max.clone();
                        current_step = Step::Found(i);
                    }
                }
                Discriminant::Empty => (),
                Discriminant::End => return current_step,
            }
        }
        unreachable!()
    }
}

/// Find a specific value in a sorted tree
pub struct Member<'a, K>(pub &'a K);

impl<'a, S, C, A, K> Walker<S, C, A> for Member<'a, K>
where
    C: Compound<S, A>,
    C::Leaf: Clone + Archive + Ord + Keyed<K>,
    <C::Leaf as Archive>::Archived: Keyed<K>,
    K: PartialEq + PartialOrd,
    A: Borrow<MaxKey<K>>,
{
    fn walk(&mut self, walk: impl Walkable<S, C, A>) -> Step {
        for i in 0.. {
            println!("probing {}", i);
            match walk.probe(i) {
                Discriminant::Empty => (),
                Discriminant::Leaf(leaf) => {
                    let key = leaf.key();
                    if key == self.0 {
                        return Step::Found(i);
                    }
                }
                Discriminant::Annotation(a) => {
                    let max: &MaxKey<K> = (*a).borrow();
                    if max >= self.0 {
                        return Step::Found(i);
                    }
                }
                Discriminant::End => {
                    return Step::Abort;
                }
            }
        }
        unreachable!()
    }
}
