// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::mem;
use core::ops::{Deref, DerefMut};

use alloc::vec::Vec;

use crate::backend::Getable;
use crate::compound::{Child, ChildMut, Compound};
use crate::link::LinkCompoundMut;
use crate::walk::{AllLeaves, Step, Walk, Walker};

#[derive(Debug)]
enum LevelNodeMut<'a, C, A> {
    Root(&'a mut C),
    Val(LinkCompoundMut<'a, C, A>),
}

impl<'a, C, A> Deref for LevelNodeMut<'a, C, A> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        match self {
            LevelNodeMut::Root(root) => root,
            LevelNodeMut::Val(val) => &**val,
        }
    }
}

impl<'a, C, A> DerefMut for LevelNodeMut<'a, C, A>
where
    C: Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            LevelNodeMut::Root(target) => *target,
            LevelNodeMut::Val(val) => val,
        }
    }
}

#[derive(Debug)]
pub struct LevelMut<'a, C, A> {
    offset: usize,
    node: LevelNodeMut<'a, C, A>,
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
    fn new_root(root: &'a mut C) -> LevelMut<'a, C, A> {
        LevelMut {
            offset: 0,
            node: LevelNodeMut::Root(root),
        }
    }

    fn new_val(link_compound: LinkCompoundMut<'a, C, A>) -> LevelMut<'a, C, A> {
        LevelMut {
            offset: 0,
            node: LevelNodeMut::Val(link_compound),
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

pub struct PartialBranchMut<'a, C, A>(Vec<LevelMut<'a, C, A>>);

impl<'a, C, A> PartialBranchMut<'a, C, A> {
    fn new(root: &'a mut C) -> Self {
        PartialBranchMut(vec![LevelMut::new_root(root)])
    }

    pub fn depth(&self) -> usize {
        self.0.len()
    }

    fn top(&self) -> &LevelMut<C, A> {
        self.0.last().expect("Never empty")
    }

    fn top_mut(&mut self) -> &mut LevelMut<'a, C, A> {
        self.0.last_mut().expect("Never empty")
    }

    fn leaf(&self) -> Option<&C::Leaf>
    where
        C: Compound<A>,
    {
        let top = self.top();
        let ofs = top.offset();

        match top.child(ofs) {
            Child::Leaf(l) => Some(l),
            _ => None,
        }
    }

    fn leaf_mut(&mut self) -> Option<&mut C::Leaf>
    where
        C: Compound<A> + Clone,
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
        if self.0.len() > 1 {
            self.0.pop()
        } else {
            None
        }
    }

    fn walk<W>(&mut self, walker: &mut W) -> Option<()>
    where
        C: Compound<A> + Getable + Clone,
        W: Walker<C, A>,
    {
        enum State<C> {
            Init,
            Push(C),
            Pop,
        }

        let mut state = State::Init;
        loop {
            match mem::replace(&mut state, State::Init) {
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
            let step = walker.walk(Walk::new(&**top, top.offset()));

            match step {
                Step::Found(walk_ofs) => {
                    *top.offset_mut() += walk_ofs;
                    return Some(());
                }
                Step::Into(walk_ofs) => {
                    *top.offset_mut() += walk_ofs;
                    let ofs = top.offset();
                    let top_child = top.child_mut(ofs);
                    if let ChildMut::Node(n) = top_child {
                        let level: LevelMut<'_, C, A> =
                            LevelMut::new_val(n.inner_mut());

                        // Extend the lifetime of the Level.
                        // See comment in `Branch::walk` for justification.
                        let extended: LevelMut<'a, C, A> =
                            unsafe { core::mem::transmute(level) };
                        state = State::Push(extended);
                    } else {
                        panic!("Attempted descent into non-node")
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
    {
        MappedBranchMut {
            inner: self,
            closure,
        }
    }

    /// Performs a tree walk, returning either a valid branch or None if the
    /// walk failed.
    pub fn walk<W>(root: &'a mut C, mut walker: W) -> Option<Self>
    where
        C: Compound<A> + Getable + Clone,
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
pub struct BranchMut<'a, C, A>(PartialBranchMut<'a, C, A>);

impl<'a, C, A> Deref for BranchMut<'a, C, A>
where
    C: Compound<A>,
{
    type Target = C::Leaf;

    fn deref(&self) -> &Self::Target {
        self.0.leaf().expect("Invalid branch")
    }
}

impl<'a, C, A> DerefMut for BranchMut<'a, C, A>
where
    C: Compound<A> + Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.leaf_mut().expect("Invalid branch")
    }
}

/// A `BranchMut` with a mapped leaf
pub struct MappedBranchMut<'a, C, A, M>
where
    C: Compound<A>,
{
    inner: BranchMut<'a, C, A>,
    closure: for<'b> fn(&'b mut C::Leaf) -> &'b mut M,
}

impl<'a, C, A, M> Deref for MappedBranchMut<'a, C, A, M>
where
    C: Compound<A> + Clone,
{
    type Target = M;

    fn deref(&self) -> &M {
        // This is safe since we never use the mutable pointer as such, and
        // convert it back to a shared reference as we return.
        unsafe {
            let transmuted: *mut Self = core::mem::transmute(self);
            (self.closure)(&mut (*transmuted).inner)
        }
    }
}

impl<'a, C, A, M> DerefMut for MappedBranchMut<'a, C, A, M>
where
    C: Compound<A> + Clone,
{
    fn deref_mut(&mut self) -> &mut M {
        (self.closure)(&mut *self.inner)
    }
}

// iterators

pub enum BranchMutIterator<'a, C, A, W>
where
    C: Compound<A> + Clone,
{
    Initial(BranchMut<'a, C, A>, W),
    Intermediate(BranchMut<'a, C, A>, W),
    Exhausted,
}

impl<'a, C, A> IntoIterator for BranchMut<'a, C, A>
where
    C: Compound<A> + Getable + Clone,
{
    type Item = &'a mut C::Leaf;

    type IntoIter = BranchMutIterator<'a, C, A, AllLeaves>;

    fn into_iter(self) -> Self::IntoIter {
        BranchMutIterator::Initial(self, AllLeaves)
    }
}

impl<'a, C, A, W> Iterator for BranchMutIterator<'a, C, A, W>
where
    C: Compound<A> + Getable + Clone,
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
                let leaf: &mut C::Leaf = &mut *branch;
                let leaf_extended: &'a mut C::Leaf =
                    unsafe { core::mem::transmute(leaf) };
                Some(leaf_extended)
            }
            _ => unreachable!(),
        }
    }
}

pub enum MappedBranchMutIterator<'a, C, A, W, M>
where
    C: Compound<A>,
{
    Initial(MappedBranchMut<'a, C, A, M>, W),
    Intermediate(MappedBranchMut<'a, C, A, M>, W),
    Exhausted,
}

impl<'a, C, A, M> IntoIterator for MappedBranchMut<'a, C, A, M>
where
    C: Compound<A> + Getable + Clone,
    M: 'a,
{
    type Item = &'a mut M;

    type IntoIter = MappedBranchMutIterator<'a, C, A, AllLeaves, M>;

    fn into_iter(self) -> Self::IntoIter {
        MappedBranchMutIterator::Initial(self, AllLeaves)
    }
}

impl<'a, C, A, W, M> Iterator for MappedBranchMutIterator<'a, C, A, W, M>
where
    C: Compound<A> + Getable + Clone,
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
                let leaf: &mut M = &mut *branch;
                let leaf_extended: &'a mut M =
                    unsafe { core::mem::transmute(leaf) };
                Some(leaf_extended)
            }
            _ => unreachable!(),
        }
    }
}
