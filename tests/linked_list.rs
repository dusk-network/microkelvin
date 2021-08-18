// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use microkelvin::{
    Annotation, Child, ChildMut, Compound, First, Link, LinkError,
    MutableLeaves,
};

#[derive(Clone, Debug)]
pub enum LinkedList<T, A>
where
    A: Annotation<T>,
{
    Empty,
    Node { val: T, next: Link<Self, A> },
}

impl<T, A> Default for LinkedList<T, A>
where
    A: Annotation<T>,
{
    fn default() -> Self {
        LinkedList::Empty
    }
}

impl<T, A> Compound<A> for LinkedList<T, A>
where
    T: Clone,
    A: Annotation<T> + Clone,
{
    type Leaf = T;

    fn child(&self, ofs: usize) -> Child<Self, A>
    where
        A: Annotation<Self::Leaf>,
    {
        match (self, ofs) {
            (LinkedList::Node { val, .. }, 0) => Child::Leaf(val),
            (LinkedList::Node { next, .. }, 1) => Child::Node(next),
            (LinkedList::Node { .. }, _) => Child::EndOfNode,
            (LinkedList::Empty, _) => Child::EndOfNode,
        }
    }

    fn child_mut(&mut self, ofs: usize) -> ChildMut<Self, A>
    where
        A: Annotation<Self::Leaf>,
    {
        match (self, ofs) {
            (LinkedList::Node { val, .. }, 0) => ChildMut::Leaf(val),
            (LinkedList::Node { next, .. }, 1) => ChildMut::Node(next),
            (LinkedList::Node { .. }, _) => ChildMut::EndOfNode,
            (LinkedList::Empty, _) => ChildMut::EndOfNode,
        }
    }
}

impl<T, A> MutableLeaves for LinkedList<T, A> where A: Annotation<T> {}

impl<T, A> LinkedList<T, A>
where
    T: Clone,
    A: Clone + Annotation<T>,
{
    pub fn new() -> Self {
        Default::default()
    }

    pub fn push(&mut self, t: T) {
        match core::mem::take(self) {
            LinkedList::Empty => {
                *self = LinkedList::Node {
                    val: t,
                    next: Link::new(LinkedList::<T, A>::Empty),
                }
            }
            old @ LinkedList::Node { .. } => {
                *self = LinkedList::Node {
                    val: t,
                    next: Link::new(old),
                };
            }
        }
    }

    pub fn pop(&mut self) -> Result<Option<T>, LinkError> {
        match core::mem::take(self) {
            LinkedList::Empty => Ok(None),
            LinkedList::Node { val: t, next } => {
                *self = next.into_compound()?;
                Ok(Some(t))
            }
        }
    }
}

#[test]
fn push() {
    let n: u64 = 1024;

    let mut list = LinkedList::<_, ()>::new();

    for i in 0..n {
        list.push(i)
    }
}

#[test]
fn push_cardinality() {
    let n: u64 = 1024;

    use microkelvin::Cardinality;

    let mut list = LinkedList::<_, Cardinality>::new();

    for i in 0..n {
        list.push(i)
    }
}

#[test]
fn push_nth() -> Result<(), LinkError> {
    let n: u64 = 1024;

    use microkelvin::{Cardinality, Nth};

    let mut list = LinkedList::<_, Cardinality>::new();

    for i in 0..n {
        list.push(i)
    }

    for i in 0..n {
        assert_eq!(*list.nth(i)?.expect("Some(branch)"), n - i - 1)
    }

    Ok(())
}

#[test]
fn push_pop() -> Result<(), LinkError> {
    let n: u64 = 1024;

    let mut list = LinkedList::<_, ()>::new();

    for i in 0..n {
        list.push(i)
    }

    for i in 0..n {
        assert_eq!(list.pop()?, Some(n - i - 1))
    }

    Ok(())
}

#[test]
fn push_mut() -> Result<(), LinkError> {
    let n: u64 = 1024;

    use microkelvin::{Cardinality, Nth};

    let mut list = LinkedList::<_, Cardinality>::new();

    for i in 0..n {
        list.push(i)
    }

    for i in 0..n {
        *list.nth_mut(i)?.expect("Some(branch)") += 1
    }

    for i in 0..n {
        assert_eq!(*list.nth(i)?.expect("Some(branch)"), n - i)
    }

    Ok(())
}

#[test]
fn iterate_immutable() -> Result<(), LinkError> {
    let n: u64 = 16;

    use microkelvin::{Cardinality, Nth};

    let mut list = LinkedList::<_, Cardinality>::new();

    for i in 0..n {
        list.push(i)
    }

    // branch from first element
    let branch = list.first()?.expect("Some(branch)");

    let mut count = n;

    for res_leaf in branch {
        let leaf = res_leaf?;

        count -= 1;

        assert_eq!(*leaf, count);
    }

    // branch from 7th element
    let branch = list.nth(6)?.expect("Some(branch)");

    let mut count = n - 6;

    for res_leaf in branch {
        let leaf = res_leaf?;

        count -= 1;

        assert_eq!(*leaf, count);
    }

    Ok(())
}

#[test]
fn iterate_mutable() -> Result<(), LinkError> {
    let n: u64 = 32;

    use microkelvin::{Cardinality, Nth};

    let mut list = LinkedList::<_, Cardinality>::new();

    for i in 0..n {
        list.push(i)
    }

    // branch from first element
    let branch_mut = list.first_mut()?.expect("Some(branch_mut)");

    let mut count = n;

    for res_leaf in branch_mut {
        *res_leaf? += 1;
    }

    // branch from first element
    let branch = list.first()?.expect("Some(brach)");

    for res_leaf in branch {
        let leaf = res_leaf?;

        assert_eq!(*leaf, count);

        count -= 1;
    }

    // branch from 8th element
    let branch = list.nth(7)?.expect("Some(branch)");

    let mut count = n - 7;

    for res_leaf in branch {
        let leaf = res_leaf?;

        assert_eq!(*leaf, count);

        count -= 1;
    }

    Ok(())
}

#[test]
fn iterate_map() -> Result<(), LinkError> {
    let n: u64 = 32;

    let mut list = LinkedList::<_, ()>::new();

    for i in 0..n {
        list.push(i)
    }

    // branch from first element
    let branch_mut = list.first()?.expect("Some(brach_mut)");
    let mapped = branch_mut.map_leaf(|x| x);

    let mut count = n - 1;

    for leaf in mapped {
        let leaf = leaf?;

        assert_eq!(*leaf, count);

        count = count.saturating_sub(1);
    }

    Ok(())
}

#[test]
fn iterate_map_mutable() -> Result<(), LinkError> {
    let n: u64 = 32;

    let mut list = LinkedList::<_, ()>::new();

    for i in 0..n {
        list.push(i)
    }

    // branch from first element
    let branch_mut = list.first_mut()?.expect("Some(branch_mut)");
    let mapped = branch_mut.map_leaf(|x| x);

    let mut count = n - 1;

    for leaf in mapped {
        let leaf = leaf?;

        assert_eq!(*leaf, count);

        count = count.saturating_sub(1);
    }

    Ok(())
}

#[test]
fn deref_mapped_mutable_branch() -> Result<(), LinkError> {
    let n: u64 = 32;

    let mut list = LinkedList::<_, ()>::new();

    for i in 0..n {
        list.push(i)
    }

    // branch from first element
    let branch_mut = list.first_mut()?.expect("Some(brach_mut)");
    let mapped = branch_mut.map_leaf(|x| x);

    assert_eq!(core::ops::Deref::deref(&mapped), &31);

    Ok(())
}
