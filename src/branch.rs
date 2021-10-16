// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::ops::Deref;
use std::marker::PhantomData;

use alloc::vec::Vec;
use rkyv::Archive;

use crate::annotations::Annotation;
use crate::compound::{ArchivedCompound, Child, Compound};
use crate::walk::{AllLeaves, Slot, Slots, Step, Walker};

pub enum LevelNode<'a, C>
where
    C: Archive,
{
    Memory(&'a C),
    Archived(&'a C::Archived),
}

pub struct Level<'a, C, A>
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
{
    offset: usize,
    // pub to be accesible from `walk.rs`
    pub(crate) node: LevelNode<'a, C>,
    _marker: PhantomData<A>,
}

impl<'a, C, A> Level<'a, C, A>
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
{
    pub fn new(root: LevelNode<'a, C>) -> Level<'a, C, A> {
        Level {
            offset: 0,
            node: root,
            _marker: PhantomData,
        }
    }

    /// Returns the offset of the branch level
    pub fn offset(&self) -> usize {
        self.offset
    }

    fn offset_mut(&mut self) -> &mut usize {
        &mut self.offset
    }
}

impl<'a, C, A> Slots<C, A> for &Level<'a, C, A>
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
{
    fn slot(&self, ofs: usize) -> Slot<C, A> {
        let child = match &self.node {
            LevelNode::Memory(root) => root.child(self.offset + ofs),
            LevelNode::Archived(_refr) => todo!(),
        };

        match child {
            Child::Leaf(l) => Slot::Leaf(l),
            Child::Node(n) => Slot::Annotation(n.annotation()),
            Child::Empty => return Slot::Empty,
            Child::EndOfNode => return Slot::End,
        }
    }
}

pub struct PartialBranch<'a, C, A>(Vec<Level<'a, C, A>>)
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>;

impl<'a, C, A> PartialBranch<'a, C, A>
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
{
    fn new(root: LevelNode<'a, C>) -> Self {
        PartialBranch(vec![Level::new(root)])
    }

    pub fn depth(&self) -> usize {
        self.0.len()
    }

    pub fn levels(&self) -> &[Level<C, A>] {
        &self.0
    }

    fn leaf(&self) -> Option<&C::Leaf>
    where
        C: Compound<A>,
        A: Annotation<C::Leaf>,
    {
        let top = self.top();
        let ofs = top.offset();

        match &top.node {
            LevelNode::Memory(root) => match root.child(ofs) {
                Child::Leaf(l) => Some(l),
                _ => None,
            },
            LevelNode::Archived(_) => todo!(),
        }
    }

    fn top(&self) -> &Level<C, A> {
        self.0.last().expect("Never empty")
    }

    fn top_mut(&mut self) -> &mut Level<'a, C, A> {
        self.0.last_mut().expect("Never empty")
    }

    fn advance(&mut self) {
        *self.top_mut().offset_mut() += 1;
    }

    fn pop(&mut self) -> Option<Level<'a, C, A>> {
        // We never pop the root
        if self.0.len() > 1 {
            self.0.pop()
        } else {
            None
        }
    }

    fn walk<W>(&mut self, walker: &mut W) -> Option<()>
    where
        W: Walker<C, A>,
    {
        enum State<'a, C, A>
        where
            C: Archive + Compound<A>,
            C::Archived: ArchivedCompound<C, A>,
            A: Annotation<C::Leaf>,
        {
            Init,
            Push(Level<'a, C, A>),
            Pop,
        }

        let mut state = State::Init;
        loop {
            match core::mem::replace(&mut state, State::Init) {
                State::Init => (),
                State::Push(push) => self.0.push(push),
                State::Pop => match self.pop() {
                    Some(_) => {
                        self.advance();
                    }
                    None => return None,
                },
            }

            let top = self.top_mut();

            match walker.walk(&*top) {
                Step::Found(walk_ofs) => {
                    *top.offset_mut() += walk_ofs;
                    let ofs = top.offset();

                    let child = match &top.node {
                        LevelNode::Memory(root) => root.child(ofs),
                        LevelNode::Archived(refr) => refr.child(ofs),
                    };

                    match child {
                        Child::Leaf(_) => return Some(()),
                        Child::Node(node) => match node.inner() {
                            crate::link::NodeRef::Memory(c) => {
                                let level = Level::new(LevelNode::Memory(c));
                                let extended: Level<'a, C, A> =
                                    unsafe { core::mem::transmute(level) };
                                state = State::Push(extended);
                            }
                            crate::link::NodeRef::Archived(_) => todo!(),
                        },
                        _ => panic!("Invalid child found"),
                    }
                }
                Step::Advance => state = State::Pop,
                Step::Abort => {
                    return None;
                }
            }
        }
    }
}

impl<'a, C, A> Branch<'a, C, A>
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
{
    /// Returns the depth of the branch
    pub fn depth(&self) -> usize {
        self.0.depth()
    }

    /// Returns a slice into the levels of the tree.
    pub fn levels(&self) -> &[Level<C, A>] {
        self.0.levels()
    }

    /// Returns a branch that maps the leaf to a specific value.
    /// Used in maps for example, to get easy access to the value of the KV-pair
    pub fn map_leaf<M>(
        self,
        closure: for<'b> fn(&'b C::Leaf) -> &'b M,
    ) -> MappedBranch<'a, C, A, M>
    where
        C: Compound<A>,
        A: Annotation<C::Leaf>,
    {
        MappedBranch {
            inner: self,
            closure,
        }
    }

    /// Performs a tree walk, returning either a valid branch or None if the
    /// walk failed.
    pub fn walk<W>(root: &'a C, mut walker: W) -> Option<Self>
    where
        C: Compound<A>,
        A: Annotation<C::Leaf>,
        W: Walker<C, A>,
    {
        let mut partial = PartialBranch::new(LevelNode::Memory(root));
        partial.walk(&mut walker).map(|()| Branch(partial))
    }

    /// Performs a tree walk, returning either a valid branch or None if the
    /// walk failed.
    pub fn walk_archived<W>(
        root: &'a C::Archived,
        mut walker: W,
    ) -> Option<Self>
    where
        C: Compound<A>,
        A: Annotation<C::Leaf>,
        W: Walker<C, A>,
    {
        let mut partial = PartialBranch::new(LevelNode::Archived(root));
        partial.walk(&mut walker).map(|()| Branch(partial))
    }
}

/// Reprents an immutable branch view into a collection.
///
/// Branche are always guaranteed to point at a leaf, and can be dereferenced
/// to the pointed-at leaf.
pub struct Branch<'a, C, A>(PartialBranch<'a, C, A>)
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>;

impl<'a, C, A> Deref for Branch<'a, C, A>
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
{
    type Target = C::Leaf;

    fn deref(&self) -> &Self::Target {
        self.0.leaf().expect("Invalid branch")
    }
}

pub struct MappedBranch<'a, C, A, M>
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
{
    inner: Branch<'a, C, A>,
    closure: for<'b> fn(&'b C::Leaf) -> &'b M,
}

impl<'a, C, A, M> Deref for MappedBranch<'a, C, A, M>
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
{
    type Target = M;

    fn deref(&self) -> &M {
        (self.closure)(&*self.inner)
    }
}

pub enum BranchIterator<'a, C, A, W>
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
{
    Initial(Branch<'a, C, A>, W),
    Intermediate(Branch<'a, C, A>, W),
    Exhausted,
}

// iterators
impl<'a, C, A> IntoIterator for Branch<'a, C, A>
where
    C: Archive + Compound<A>,
    C::Leaf: 'a,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
{
    type Item = &'a C::Leaf;

    type IntoIter = BranchIterator<'a, C, A, AllLeaves>;

    fn into_iter(self) -> Self::IntoIter {
        BranchIterator::Initial(self, AllLeaves)
    }
}

impl<'a, C, A, W> Iterator for BranchIterator<'a, C, A, W>
where
    C: Archive + Compound<A>,
    C::Leaf: 'a,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
    W: Walker<C, A>,
{
    type Item = &'a C::Leaf;

    fn next(&mut self) -> Option<Self::Item> {
        match core::mem::replace(self, BranchIterator::Exhausted) {
            BranchIterator::Initial(branch, walker) => {
                *self = BranchIterator::Intermediate(branch, walker);
            }
            BranchIterator::Intermediate(mut branch, mut walker) => {
                branch.0.advance();
                // access partialbranch
                match branch.0.walk(&mut walker) {
                    None => {
                        *self = BranchIterator::Exhausted;
                        return None;
                    }
                    Some(_) => {
                        *self = BranchIterator::Intermediate(branch, walker);
                    }
                }
            }
            BranchIterator::Exhausted => {
                return None;
            }
        }

        match self {
            BranchIterator::Intermediate(branch, _) => {
                let leaf: &C::Leaf = &*branch;
                let leaf_extended: &'a C::Leaf =
                    unsafe { core::mem::transmute(leaf) };
                Some(leaf_extended)
            }
            _ => unreachable!(),
        }
    }
}

pub enum MappedBranchIterator<'a, C, A, W, M>
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
{
    Initial(MappedBranch<'a, C, A, M>, W),
    Intermediate(MappedBranch<'a, C, A, M>, W),
    Exhausted,
}

impl<'a, C, A, M> IntoIterator for MappedBranch<'a, C, A, M>
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
    M: 'a,
{
    type Item = &'a M;

    type IntoIter = MappedBranchIterator<'a, C, A, AllLeaves, M>;

    fn into_iter(self) -> Self::IntoIter {
        MappedBranchIterator::Initial(self, AllLeaves)
    }
}

impl<'a, C, A, W, M> Iterator for MappedBranchIterator<'a, C, A, W, M>
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
    W: Walker<C, A>,
    M: 'a,
{
    type Item = &'a M;

    fn next(&mut self) -> Option<Self::Item> {
        match core::mem::replace(self, Self::Exhausted) {
            Self::Initial(branch, walker) => {
                *self = Self::Intermediate(branch, walker);
            }
            Self::Intermediate(mut branch, mut walker) => {
                branch.inner.0.advance();
                // access partialbranch
                match branch.inner.0.walk(&mut walker) {
                    None => {
                        *self = Self::Exhausted;
                        return None;
                    }
                    Some(_) => {
                        *self = Self::Intermediate(branch, walker);
                    }
                }
            }
            Self::Exhausted => {
                return None;
            }
        }

        match self {
            Self::Intermediate(branch, _) => {
                let leaf: &M = &*branch;
                let leaf_extended: &'a M =
                    unsafe { core::mem::transmute(leaf) };
                Some(leaf_extended)
            }
            _ => unreachable!(),
        }
    }
}
