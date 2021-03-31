// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

/// Annotation to keep track of the largest element of a collection
use super::{Ann, Annotation};
use canonical::Canon;
use canonical_derive::Canon;
use core::cmp::Ordering;

/// The maximum value of a collection
#[derive(Canon, PartialEq, Eq, Debug, Clone, Copy)]
pub enum Max<K> {
    /// Identity of max, everything else is larger
    NegativeInfinity,
    /// Actual max value
    Maximum(K),
}

/// Trait for getting the key from a leaf value
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

impl<K> Default for Max<K> {
    fn default() -> Self {
        Max::NegativeInfinity
    }
}

impl<K> PartialOrd for Max<K>
where
    K: PartialOrd,
{
    fn partial_cmp(&self, other: &Max<K>) -> Option<Ordering> {
        match (self, other) {
            (Max::NegativeInfinity, Max::NegativeInfinity) => {
                Some(Ordering::Equal)
            }
            (_, Max::NegativeInfinity) => Some(Ordering::Less),
            (Max::NegativeInfinity, _) => Some(Ordering::Greater),
            (Max::Maximum(a), Max::Maximum(b)) => a.partial_cmp(b),
        }
    }
}

impl<K> Ord for Max<K>
where
    K: Ord,
{
    fn cmp(&self, other: &Max<K>) -> Ordering {
        match (self, other) {
            (Max::NegativeInfinity, Max::NegativeInfinity) => Ordering::Equal,
            (_, Max::NegativeInfinity) => Ordering::Less,
            (Max::NegativeInfinity, _) => Ordering::Greater,
            (Max::Maximum(a), Max::Maximum(b)) => a.cmp(b),
        }
    }
}

impl<K, L> Annotation<L> for Max<K>
where
    L: Keyed<K>,
    K: Ord + Clone,
{
    fn from_leaf(leaf: &L) -> Self {
        Max::Maximum(leaf.key().clone())
    }

    fn combine(annotations: &[Ann<Self>]) -> Self {
        let mut max = Max::NegativeInfinity;
        for a in annotations {
            if **a > max {
                max = (*a).clone()
            }
        }
        max
    }
}
