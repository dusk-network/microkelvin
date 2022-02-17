// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::marker::PhantomData;
use core::mem;
use core::ops::{Deref, DerefMut};

use alloc::vec::Vec;
use rkyv::Archive;

use crate::compound::{ChildMut, Compound};
use crate::tower::{WellArchived, WellFormed};
use crate::walk::{All, Discriminant, Step, Walkable, Walker};
use crate::{Annotation, Child, MaybeArchived};

#[derive(Debug)]
pub struct LevelMut<'a, C, A> {
    offset: usize,
    node: &'a mut C,
    _marker: PhantomData<A>,
}

impl<'a, C, A> Deref for LevelMut<'a, C, A> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &*self.node
    }
}

impl<'a, C, A> DerefMut for LevelMut<'a, C, A>
where
    C: Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.node
    }
}

impl<'a, C, A> LevelMut<'a, C, A> {
    fn new(root: &'a mut C) -> LevelMut<'a, C, A> {
        LevelMut {
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

impl<'a, C, A> Walkable<C, A> for &'a LevelMut<'a, C, A>
where
    C: Compound<A> + WellFormed,
    C::Leaf: WellFormed,
    A: Annotation<C::Leaf>,
{
    fn probe(&self, ofs: usize) -> Discriminant<C::Leaf, A> {
        match self.node.child(ofs + self.offset) {
            Child::Leaf(leaf) => {
                Discriminant::Leaf(MaybeArchived::Memory(leaf))
            }
            Child::Link(link) => Discriminant::Annotation(link.annotation()),
            Child::Empty => Discriminant::Empty,
            Child::End => Discriminant::End,
        }
    }
}

#[derive(Debug)]
pub struct PartialBranchMut<'a, C, A> {
    levels: Vec<LevelMut<'a, C, A>>,
}

impl<'a, C, A> PartialBranchMut<'a, C, A> {
    fn new(root: &'a mut C) -> Self {
        PartialBranchMut {
            levels: vec![LevelMut::new(root)],
        }
    }

    pub fn depth(&self) -> usize {
        self.levels.len()
    }

    fn top_mut(&mut self) -> &mut LevelMut<'a, C, A> {
        self.levels.last_mut().expect("Never empty")
    }

    pub fn leaf_mut(&mut self) -> Option<&mut C::Leaf>
    where
        C: Compound<A> + WellFormed,
    {
        let top = self.top_mut();
        let ofs = top.offset();

        match top.child_mut(ofs) {
            ChildMut::Leaf(l) => Some(l),
            _ => None,
        }
    }

    fn advance(&mut self) {
        *self.top_mut().offset_mut() += 1
    }

    fn pop(&mut self) -> Option<LevelMut<'a, C, A>> {
        // We never pop the root
        if self.levels.len() > 1 {
            self.levels.pop()
        } else {
            None
        }
    }

    fn walk<W>(&mut self, walker: &mut W) -> Option<()>
    where
        C: Compound<A> + WellFormed,
        C::Archived: WellArchived<C>,
        A: Annotation<C::Leaf>,
        C::Leaf: WellFormed,
        W: Walker<C, A>,
    {
        enum State<Level> {
            Init,
            Push(Level),
            Pop,
        }

        let mut state = State::Init;
        loop {
            match mem::replace(&mut state, State::Init) {
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
            let step = walker.walk(&*top);

            match step {
                Step::Found(walk_ofs) => {
                    *top.offset_mut() += walk_ofs;
                    let ofs = top.offset();

                    match top.node.child_mut(ofs) {
                        ChildMut::Leaf(_) => return Some(()),
                        ChildMut::Link(link) => {
                            let inner = link.inner_mut();
                            let extended: &'a mut C =
                                unsafe { core::mem::transmute(inner) };
                            state = State::Push(LevelMut::new(extended));
                        }
                        _ => panic!("Invalid child found"),
                    }
                }
                Step::Advance => state = State::Pop,
                Step::Abort => return None,
            }
        }
    }
}

impl<'a, C, A> BranchMut<'a, C, A> {
    /// Returns the depth of the branch
    pub fn depth(&self) -> usize {
        self.0.depth()
    }

    /// Returns a branch that maps the leaf to a specific value.
    /// Used in maps for example, to get easy access to the value of the KV-pair
    pub fn map_leaf<M>(
        self,
        closure: for<'b> fn(&'b mut C::Leaf) -> &'b mut M,
    ) -> MappedBranchMut<'a, C, A, M>
    where
        C: Compound<A>,
        C::Leaf: Archive,
    {
        MappedBranchMut {
            inner: self,
            closure,
        }
    }

    /// Returns a mutable reference to the leaf pointet to by the branch
    pub fn leaf_mut(&mut self) -> &mut C::Leaf
    where
        C: Compound<A> + WellFormed,
    {
        self.0.leaf_mut().expect("invalid branch")
    }

    /// Performs a tree walk, returning either a valid branch or None if the
    /// walk failed.
    pub fn walk<W>(root: &'a mut C, mut walker: W) -> Option<Self>
    where
        C: Compound<A> + WellFormed,
        C::Archived: WellArchived<C>,
        C::Leaf: WellFormed,
        A: Annotation<C::Leaf>,
        W: Walker<C, A>,
    {
        let mut partial = PartialBranchMut::new(root);
        partial.walk(&mut walker).map(|()| BranchMut(partial))
    }
}

/// Reprents a branch view into a collection.
///
/// BranchMut allows you to manipulate the value of the leaf, but disallows
/// manipulating the branch nodes directly, to avoid breaking any datastructure
/// invariants.
///
/// Branches are always guaranteed to point at a leaf, and can be dereferenced
/// to the pointed-at leaf.
#[derive(Debug)]
pub struct BranchMut<'a, C, A>(PartialBranchMut<'a, C, A>);

/// A `BranchMut` with a mapped leaf
pub struct MappedBranchMut<'a, C, A, M>
where
    C: Compound<A>,
{
    inner: BranchMut<'a, C, A>,
    closure: for<'b> fn(&'b mut C::Leaf) -> &'b mut M,
}

impl<'a, C, A, M> MappedBranchMut<'a, C, A, M>
where
    C: Compound<A> + WellFormed,
{
    /// Returns a mutable reference to the mapped value
    pub fn leaf_mut(&mut self) -> &mut M {
        (self.closure)(self.inner.leaf_mut())
    }
}

// iterators

#[derive(Debug)]
pub enum BranchMutIterator<'a, C, A, W> {
    Initial(BranchMut<'a, C, A>, W),
    Intermediate(BranchMut<'a, C, A>, W),
    Exhausted,
}

impl<'a, C, A> IntoIterator for BranchMut<'a, C, A>
where
    C: Compound<A> + WellFormed,
    C::Leaf: 'a + WellFormed,
    C::Archived: WellArchived<C>,
    A: Annotation<C::Leaf>,
{
    type Item = &'a mut C::Leaf;

    type IntoIter = BranchMutIterator<'a, C, A, All>;

    fn into_iter(self) -> Self::IntoIter {
        BranchMutIterator::Initial(self, All)
    }
}

impl<'a, C, A, W> Iterator for BranchMutIterator<'a, C, A, W>
where
    C: Compound<A> + WellFormed,
    C::Leaf: 'a + WellFormed,
    C::Archived: WellArchived<C>,
    A: Annotation<C::Leaf>,
    W: Walker<C, A>,
{
    type Item = &'a mut C::Leaf;

    fn next(&mut self) -> Option<Self::Item> {
        match core::mem::replace(self, BranchMutIterator::Exhausted) {
            BranchMutIterator::Initial(branch, walker) => {
                *self = BranchMutIterator::Intermediate(branch, walker);
            }
            BranchMutIterator::Intermediate(mut branch, mut walker) => {
                branch.0.advance();
                // access partialbranch
                match branch.0.walk(&mut walker) {
                    None => {
                        *self = BranchMutIterator::Exhausted;
                        return None;
                    }
                    Some(_) => {
                        *self = BranchMutIterator::Intermediate(branch, walker);
                    }
                }
            }
            BranchMutIterator::Exhausted => {
                return None;
            }
        }

        match self {
            BranchMutIterator::Intermediate(branch, _) => {
                let leaf: &mut C::Leaf = branch.leaf_mut();
                let leaf_extended: &'a mut C::Leaf =
                    unsafe { core::mem::transmute(leaf) };
                Some(leaf_extended)
            }
            _ => unreachable!(),
        }
    }
}

pub enum MappedBranchMutIterator<'a, C, A, M, W>
where
    C: Compound<A>,
{
    Initial(MappedBranchMut<'a, C, A, M>, W),
    Intermediate(MappedBranchMut<'a, C, A, M>, W),
    Exhausted,
}

impl<'a, C, A, M> IntoIterator for MappedBranchMut<'a, C, A, M>
where
    C: Compound<A> + WellFormed,
    C::Archived: WellArchived<C>,
    C::Leaf: WellFormed,
    <C::Leaf as Archive>::Archived: WellArchived<C::Leaf>,
    A: Annotation<C::Leaf>,
    M: 'a,
{
    type Item = &'a mut M;

    type IntoIter = MappedBranchMutIterator<'a, C, A, M, All>;

    fn into_iter(self) -> Self::IntoIter {
        MappedBranchMutIterator::Initial(self, All)
    }
}

impl<'a, C, A, M, W> Iterator for MappedBranchMutIterator<'a, C, A, M, W>
where
    C: Compound<A> + WellFormed,
    C::Archived: WellArchived<C>,
    C::Leaf: WellFormed,
    <C::Leaf as Archive>::Archived: WellArchived<C::Leaf>,
    A: Annotation<C::Leaf>,
    W: Walker<C, A>,
    M: 'a,
{
    type Item = &'a mut M;

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
                let leaf: &mut M = (branch.closure)(branch.inner.leaf_mut());
                let leaf_extended: &'a mut M =
                    unsafe { core::mem::transmute(leaf) };
                Some(leaf_extended)
            }
            _ => unreachable!(),
        }
    }
}

/// A trait for refering to a branch based solely on its leaf type
pub trait BranchRefMut<'a, T>
where
    T: Archive,
{
    /// Provides a reference to the leaf of the branch
    fn leaf_mut(&mut self) -> &mut T;
}

impl<'a, C, A, T> BranchRefMut<'a, T> for MappedBranchMut<'a, C, A, T>
where
    C: Compound<A> + WellFormed,
    T: WellFormed,
{
    fn leaf_mut(&mut self) -> &mut T {
        MappedBranchMut::leaf_mut(self)
    }
}
