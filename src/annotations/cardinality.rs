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
use crate::walk::{Discriminant, Step, Walkable, Walker};
use crate::Compound;

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
    fn combine(&mut self, other: &A) {
        self.0 += other.borrow().0
    }
}

/// Walker method to find the nth element of a compound collection
#[derive(Debug)]
pub struct Nth(pub u64);

impl<C, A, S> Walker<C, A, S> for Nth
where
    C: Compound<A, S>,
    A: Borrow<Cardinality>,
{
    fn walk(&mut self, walk: impl Walkable<C, A, S>) -> Step {
        for i in 0.. {
            match walk.probe(i) {
                Discriminant::Leaf(_) => {
                    if self.0 == 0 {
                        return Step::Found(i);
                    } else {
                        self.0 -= 1;
                    }
                }
                Discriminant::Annotation(a) => {
                    let card: &Cardinality = (*a).borrow();
                    if card.0 <= self.0 {
                        self.0 -= u64::from(card.0);
                    } else {
                        return Step::Found(i);
                    }
                }
                Discriminant::Empty => (),
                Discriminant::End => return Step::Abort,
            };
        }
        unreachable!()
    }
}
