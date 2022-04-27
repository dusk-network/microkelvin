// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

/// Annotation to keep track of the largest element of a collection
use core::borrow::Borrow;
use core::cmp::Ordering;
use core::marker::PhantomData;

use bytecheck::CheckBytes;
use rkyv::{Archive, Deserialize, Serialize};

use crate::annotations::{Annotation, Combine};
use crate::walk::{Discriminant, Step, Walkable, Walker};
use crate::{Compound, Fundamental};

/// The maximum value of a collection
#[derive(Clone, Debug, Archive, Serialize, Deserialize, CheckBytes)]
#[repr(u8)]
#[archive(as = "Self")]
#[archive(bound(archive = "
  K: Fundamental"))]
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

impl<K, O> PartialEq<O> for MaxKey<K>
where
    K: Borrow<O>,
    O: PartialEq,
{
    fn eq(&self, other: &O) -> bool {
        match self {
            MaxKey::NegativeInfinity => false,
            MaxKey::Maximum(k) => k.borrow() == other,
        }
    }
}

impl<K, O> PartialOrd<O> for MaxKey<K>
where
    K: Borrow<O>,
    O: PartialOrd + PartialEq,
{
    fn partial_cmp(&self, other: &O) -> Option<Ordering> {
        match self {
            MaxKey::NegativeInfinity => Some(Ordering::Less),
            MaxKey::Maximum(k) => k.borrow().partial_cmp(other),
        }
    }
}

impl<K, L> Annotation<L> for MaxKey<K>
where
    L: Keyed<K>,
    K: Fundamental + Ord,
{
    fn from_leaf(leaf: &L) -> Self {
        MaxKey::Maximum(leaf.key().clone())
    }
}

impl<K, A> Combine<A> for MaxKey<K>
where
    K: Ord + Clone,
    A: Borrow<Self>,
{
    fn combine(&mut self, other: &A) {
        let b = other.borrow();
        match (&*self, b) {
            (MaxKey::NegativeInfinity, MaxKey::Maximum(m))
            | (MaxKey::Maximum(m), MaxKey::NegativeInfinity) => {
                *self = MaxKey::Maximum(m.clone())
            }
            (MaxKey::Maximum(a), MaxKey::Maximum(b)) => {
                if b > a {
                    *self = MaxKey::Maximum(b.clone())
                }
            }
            _ => (),
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

impl<C, A, K> Walker<C, A> for FindMaxKey<K>
where
    C: Compound<A>,
    C::Leaf: Archive + Keyed<K>,
    <C::Leaf as Archive>::Archived: Keyed<K>,
    A: Borrow<MaxKey<K>>,
    K: Ord + Clone,
{
    fn walk(&mut self, walk: impl Walkable<C, A>) -> Step {
        let mut current_max: MaxKey<K> = MaxKey::NegativeInfinity;
        let mut current_step = Step::Abort;

        for i in 0.. {
            match walk.probe(i) {
                Discriminant::Leaf(l) => {
                    if current_max < *l.key() {
                        current_max = MaxKey::Maximum(l.key().clone());
                        current_step = Step::Found(i);
                    }
                }
                Discriminant::Annotation(ann) => {
                    let node_max: &MaxKey<K> = (*ann).borrow();

                    match (&current_max, node_max) {
                        (
                            MaxKey::NegativeInfinity,
                            max @ MaxKey::Maximum(_),
                        ) => {
                            current_max = max.clone();
                            current_step = Step::Found(i);
                        }
                        (MaxKey::Maximum(_), MaxKey::NegativeInfinity) => (),
                        (MaxKey::Maximum(a), max @ MaxKey::Maximum(b))
                            if b > a =>
                        {
                            current_max = max.clone();
                            current_step = Step::Found(i);
                        }
                        _ => (),
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

impl<'a, C, A, K> Walker<C, A> for Member<'a, K>
where
    C: Compound<A>,
    C::Leaf: Clone + Archive + Ord + Keyed<K>,
    <C::Leaf as Archive>::Archived: Keyed<K>,
    K: PartialEq + PartialOrd,
    A: Borrow<MaxKey<K>>,
{
    fn walk(&mut self, walk: impl Walkable<C, A>) -> Step {
        for i in 0.. {
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
