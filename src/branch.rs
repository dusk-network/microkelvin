// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::marker::PhantomData;

use alloc::vec::Vec;
use rkyv::Archive;

use crate::annotations::{ARef, Annotation};
use crate::compound::{ArchivedChild, ArchivedCompound, Child, Compound};
use crate::walk::{First, Slot, Slots, Step, Walker};
use crate::wrappers::AWrap;

pub struct Level<'a, C, A>
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
{
    offset: usize,
    // pub to be accesible from `walk.rs`
    pub(crate) node: AWrap<'a, C>,
    _marker: PhantomData<A>,
}

impl<'a, C, A> Level<'a, C, A>
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
{
    pub fn new(root: AWrap<'a, C>) -> Level<'a, C, A> {
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
        match &self.node {
            AWrap::Memory(root) => match root.child(self.offset + ofs) {
                Child::Leaf(l) => Slot::Leaf(l),
                Child::Node(n) => Slot::Annotation(n.annotation()),
                Child::Empty => Slot::Empty,
                Child::EndOfNode => Slot::End,
            },
            AWrap::Archived(arch) => match arch.child(self.offset + ofs) {
                ArchivedChild::Leaf(l) => Slot::ArchivedLeaf(l),
                ArchivedChild::Node(n) => {
                    Slot::Annotation(ARef::Borrowed(n.annotation()))
                }
                ArchivedChild::Empty => Slot::Empty,
                ArchivedChild::EndOfNode => Slot::End,
            },
        }
    }
}

pub struct PartialBranch<'a, C, A>
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
{
    levels: Vec<Level<'a, C, A>>,
}

impl<'a, C, A> PartialBranch<'a, C, A>
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
{
    fn new(root: AWrap<'a, C>) -> Self {
        PartialBranch {
            levels: vec![Level::new(root)],
        }
    }

    pub fn depth(&self) -> usize {
        self.levels.len()
    }

    pub fn levels(&self) -> &[Level<C, A>] {
        &self.levels
    }

    fn leaf(&self) -> Option<AWrap<C::Leaf>>
    where
        C: Compound<A>,
        A: Annotation<C::Leaf>,
    {
        let top = self.top();
        let ofs = top.offset();

        match &top.node {
            AWrap::Memory(root) => match root.child(ofs) {
                Child::Leaf(l) => Some(AWrap::Memory(l)),
                _ => None,
            },
            AWrap::Archived(arch) => match arch.child(ofs) {
                ArchivedChild::Leaf(l) => Some(AWrap::Archived(l)),
                _ => None,
            },
        }
    }

    fn top(&self) -> &Level<C, A> {
        self.levels.last().expect("Never empty")
    }

    fn top_mut(&mut self) -> &mut Level<'a, C, A> {
        self.levels.last_mut().expect("Never empty")
    }

    fn advance(&mut self) {
        *self.top_mut().offset_mut() += 1;
    }

    fn pop(&mut self) -> Option<Level<'a, C, A>> {
        // We never pop the root
        if self.levels.len() > 1 {
            self.levels.pop()
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
                State::Push(push) => self.levels.push(push),
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

                    let ca = match &top.node {
                        AWrap::Memory(root) => match root.child(ofs) {
                            Child::Leaf(_) => return Some(()),
                            Child::Node(node) => match node.inner() {
                                AWrap::Memory(c) => {
                                    let level = Level::new(AWrap::Memory(c));
                                    let extended: Level<'a, C, A> =
                                        unsafe { core::mem::transmute(level) };
                                    state = State::Push(extended);
                                    continue;
                                }
                                AWrap::Archived(ca) => ca,
                            },
                            _ => panic!("Invalid child found"),
                        },
                        AWrap::Archived(arch) => match arch.child(ofs) {
                            ArchivedChild::Leaf(_) => return Some(()),
                            ArchivedChild::Node(node) => node.inner(),
                            _ => panic!("Invalid child found"),
                        },
                    };
                    let level: Level<C, A> = Level::new(AWrap::Archived(ca));
                    let extended: Level<'a, C, A> =
                        unsafe { core::mem::transmute(level) };
                    state = State::Push(extended);
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
        closure: for<'b> fn(AWrap<'b, C::Leaf>) -> AWrap<'b, M>,
    ) -> MappedBranch<'a, C, A, M>
    where
        C: Compound<A>,
        A: Annotation<C::Leaf>,
        M: Archive,
    {
        MappedBranch {
            inner: self,
            closure,
        }
    }

    /// Returns a reference to the currently pointed-at leaf
    pub fn leaf(&self) -> AWrap<C::Leaf> {
        self.0.leaf().expect("Invalid branch")
    }

    /// Performs a tree walk, returning either a valid branch or None if the
    /// walk failed.
    pub fn walk<W>(root: AWrap<'a, C>, mut walker: W) -> Option<Self>
    where
        C: Compound<A>,
        A: Annotation<C::Leaf>,
        W: Walker<C, A>,
    {
        let mut partial = PartialBranch::new(root);
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

/// A branch that applies a map to its leaf
pub struct MappedBranch<'a, C, A, M>
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
    M: Archive,
{
    inner: Branch<'a, C, A>,
    closure: for<'b> fn(AWrap<'b, C::Leaf>) -> AWrap<'b, M>,
}

impl<'a, C, A, M> MappedBranch<'a, C, A, M>
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
    M: Archive,
{
    /// Get the mapped leaf of the branch
    pub fn leaf(&'a self) -> AWrap<'a, M> {
        (self.closure)(self.inner.leaf())
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
    type Item = AWrap<'a, C::Leaf>;

    type IntoIter = BranchIterator<'a, C, A, First>;

    fn into_iter(self) -> Self::IntoIter {
        BranchIterator::Initial(self, First)
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
    type Item = AWrap<'a, C::Leaf>;

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
                let leaf = branch.leaf();
                let leaf_extended: AWrap<'a, C::Leaf> =
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
    M: Archive,
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
    M: 'a + Archive,
{
    type Item = AWrap<'a, M>;

    type IntoIter = MappedBranchIterator<'a, C, A, First, M>;

    fn into_iter(self) -> Self::IntoIter {
        MappedBranchIterator::Initial(self, First)
    }
}

impl<'a, C, A, W, M> Iterator for MappedBranchIterator<'a, C, A, W, M>
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
    W: Walker<C, A>,
    M: 'a + Archive,
{
    type Item = AWrap<'a, M>;

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
                let leaf = branch.leaf();
                let leaf_extended: AWrap<'a, M> =
                    unsafe { core::mem::transmute(leaf) };
                Some(leaf_extended)
            }
            _ => unreachable!(),
        }
    }
}
