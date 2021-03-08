// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::mem;
use core::ops::{Deref, DerefMut};

use alloc::vec::Vec;

use canonical::CanonError;

use crate::annotations::{AnnRefMut, Annotated, Annotation};
use crate::compound::{Child, ChildMut, Compound};

/// The argument given to a closure to `walk` a `BranchMut`.
pub enum WalkMut<'a, C, A>
where
    C: Compound<A>,
{
    /// Walk encountered a leaf
    Leaf(&'a mut C::Leaf),
    /// Walk encountered a node
    Node(&'a mut Annotated<C, A>),
}

/// The return value from a closure to `walk` the tree.
///
/// Determines how the `BranchMut` is constructed
pub enum StepMut<'a, C, A>
where
    C: Compound<A>,
{
    /// The correct leaf was found!
    Found(&'a mut C::Leaf),
    /// Step to the next child on this level
    Next,
    /// Traverse the branch deeper
    Into(&'a mut Annotated<C, A>),
    /// Abort the search
    Abort,
}

/// Represents a level in the branch.
///
/// The offset is pointing at the child of the node stored behind the LevelInner
/// pointer.
pub struct LevelMut<C, A> {
    offset: usize,
    node: Annotated<C, A>,
}

impl<C, A> Into<Annotated<C, A>> for LevelMut<C, A> {
    fn into(self) -> Annotated<C, A> {
        self.node
    }
}

impl<C, A> LevelMut<C, A>
where
    C: Compound<A>,
{
    /// Returns the offset of the branch level
    pub fn offset(&self) -> usize {
        self.offset
    }

    fn offset_mut(&mut self) -> &mut usize {
        &mut self.offset
    }

    fn new(node: Annotated<C, A>) -> Self {
        LevelMut { offset: 0, node }
    }

    fn node(&self) -> &Annotated<C, A> {
        &self.node
    }

    fn node_mut(&mut self) -> &mut Annotated<C, A> {
        &mut self.node
    }
}

pub struct PartialBranchMut<'a, C, A> {
    root: &'a mut C,
    root_offset: usize,
    levels: Vec<LevelMut<C, A>>,
}

enum TopNodeMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    Root(&'a mut C),
    Val(AnnRefMut<'a, C, A>),
}

impl<'a, C, A> Deref for TopNodeMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        match self {
            TopNodeMut::Root(target) => target,
            TopNodeMut::Val(rc) => &*rc,
        }
    }
}

impl<'a, C, A> DerefMut for TopNodeMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            TopNodeMut::Root(target) => target,
            TopNodeMut::Val(val) => val,
        }
    }
}

impl<'a, C, A> PartialBranchMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    fn new(root: &'a mut C) -> Self {
        PartialBranchMut {
            root,
            root_offset: 0,
            levels: vec![],
        }
    }

    pub fn depth(&self) -> usize {
        1 + self.levels.len()
    }

    fn leaf(&self) -> Option<&C::Leaf> {
        match self.levels.last() {
            Some(_last) => todo!(),
            None => match self.root.child(self.root_offset) {
                Child::Leaf(leaf) => Some(leaf),
                _ => None,
            },
        }
    }

    fn leaf_mut(&mut self) -> Option<&mut C::Leaf> {
        match self.levels.last_mut() {
            Some(_last) => todo!(),
            None => match self.root.child_mut(self.root_offset) {
                ChildMut::Leaf(leaf) => Some(leaf),
                _ => None,
            },
        }
    }

    fn advance(&mut self) {
        match self.levels.last_mut() {
            Some(last) => *last.offset_mut() += 1,
            None => self.root_offset += 1,
        }
    }

    fn pop(&mut self) -> Option<()> {
        // We can safely assume that all calls to "val_mut" here succeeds,
        // since the nodes must already be in memory from when building up the
        // branch.

        // Therefore we can forego the Error handling.
        // This is neccesary to be able to call this method from a Drop
        // implementation.

        self.levels
            .pop()
            .map(|popped| match self.levels.last_mut() {
                Some(top_level) => {
                    let ofs = top_level.offset();
                    if let ChildMut::Node(to_update) = top_level
                        .node_mut()
                        .val_mut()
                        .expect("See comment above")
                        .child_mut(ofs)
                    {
                        *to_update = popped.into()
                    } else {
                        unreachable!("Invalid parent structure")
                    }
                }
                None => todo!(),
            })
    }

    fn walk<W>(&mut self, mut walker: W) -> Result<Option<()>, CanonError>
    where
        W: FnMut(WalkMut<C, A>) -> StepMut<C, A>,
    {
        enum State<C> {
            Init,
            Push(C),
            Pop,
            Advance,
        }

        let mut state = State::Init;
        loop {
            match mem::replace(&mut state, State::Init) {
                State::Init => (),
                State::Push(push) => self.levels.push(push),
                State::Pop => match self.levels.pop() {
                    Some(_) => {
                        self.advance();
                    }
                    None => return Ok(None),
                },
                State::Advance => self.advance(),
            }

            let (mut node, ofs) = match self.levels.last_mut() {
                Some(top_level) => {
                    let ofs = top_level.offset();
                    (TopNodeMut::Val(top_level.node_mut().val_mut()?), ofs)
                }
                None => (TopNodeMut::Root(self.root), self.root_offset),
            };

            let step = match node.child_mut(ofs) {
                ChildMut::Leaf(l) => walker(WalkMut::Leaf(l)),
                ChildMut::Node(c) => walker(WalkMut::Node(c)),
                ChildMut::Empty => todo!(),
                ChildMut::EndOfNode => {
                    state = State::Pop;
                    continue;
                }
            };

            match step {
                StepMut::Found(_) => {
                    return Ok(Some(()));
                }
                StepMut::Next => {
                    state = State::Advance;
                }
                StepMut::Into(n) => {
                    state = State::Push(LevelMut::new(n.clone()));
                }

                StepMut::Abort => return Ok(None),
            }
        }
    }

    fn path<P>(&mut self, mut path: P) -> Result<Option<()>, CanonError>
    where
        P: FnMut() -> usize,
    {
        let mut push = None;
        loop {
            if let Some(push) = push.take() {
                self.levels.push(push);
            }

            match self.levels.last_mut() {
                Some(top_level) => {
                    let ofs = path();
                    *top_level.offset_mut() = ofs;

                    match top_level.node().val()?.child(ofs) {
                        Child::Leaf(_) => {
                            return Ok(Some(()));
                        }
                        Child::Node(c) => push = Some(LevelMut::new(c.clone())),
                        Child::Empty => {
                            return Ok(None);
                        }
                        Child::EndOfNode => {
                            return Ok(None);
                        }
                    }
                }
                None => todo!(),
            }
        }
    }
}

impl<'a, C, A> Drop for BranchMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf> + Clone,
{
    fn drop(&mut self) {
        // unwind when dropping
        while self.0.pop().is_some() {}
    }
}

impl<'a, C, A> BranchMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf> + Clone,
{
    /// Returns the depth of the branch
    pub fn depth(&self) -> usize {
        self.0.depth()
    }

    /// Returns a branch that maps the leaf to a specific value.
    /// Used in maps for example, to get easy access to the value of the KV-pair
    pub fn map_leaf<M>(
        self,
        closure: for<'b> fn(&'b C::Leaf) -> &'b M,
    ) -> BranchMutMapped<'a, C, A, M> {
        BranchMutMapped {
            inner: self,
            closure,
        }
    }

    /// Returns a branch that maps the leaf to a specific value.
    /// Used in maps for example, to get easy access to the value of the KV-pair
    pub fn map_leaf_mut<M>(
        self,
        closure: for<'b> fn(&'b mut C::Leaf) -> &'b mut M,
    ) -> BranchMutMappedMut<'a, C, A, M> {
        BranchMutMappedMut {
            inner: self,
            closure,
        }
    }

    /// Performs a tree walk, returning either a valid branch or None if the
    /// walk failed.
    pub fn walk<W>(
        root: &'a mut C,
        walker: W,
    ) -> Result<Option<Self>, CanonError>
    where
        W: FnMut(WalkMut<C, A>) -> StepMut<C, A>,
    {
        let mut partial = PartialBranchMut::new(root);
        Ok(match partial.walk(walker)? {
            Some(()) => Some(BranchMut(partial)),
            None => None,
        })
    }

    /// Construct a branch given a function returning child offsets
    pub fn path<P>(root: &'a mut C, path: P) -> Result<Option<Self>, CanonError>
    where
        P: FnMut() -> usize,
    {
        let mut partial = PartialBranchMut::new(root);
        Ok(match partial.path(path)? {
            Some(()) => Some(BranchMut(partial)),
            None => None,
        })
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
///
/// The const generic `N` represents the maximum depth of the branch.
pub struct BranchMut<'a, C, A>(PartialBranchMut<'a, C, A>)
where
    C: Compound<A>,
    A: Annotation<C::Leaf> + Clone;

impl<'a, C, A> Deref for BranchMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf> + Clone,
{
    type Target = C::Leaf;

    fn deref(&self) -> &Self::Target {
        self.0.leaf().expect("Invalid branch")
    }
}

impl<'a, C, A> DerefMut for BranchMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf> + Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.leaf_mut().expect("Invalid branch")
    }
}

/// A `BranchMut` with a mapped leaf
pub struct BranchMutMapped<'a, C, A, M>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    inner: BranchMut<'a, C, A>,
    closure: for<'b> fn(&'b C::Leaf) -> &'b M,
}

impl<'a, C, A, M> Deref for BranchMutMapped<'a, C, A, M>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    type Target = M;

    fn deref(&self) -> &M {
        (self.closure)(&*self.inner)
    }
}

/// A `BranchMut` with a mutably mapped leaf
pub struct BranchMutMappedMut<'a, C, A, M>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    inner: BranchMut<'a, C, A>,
    closure: for<'b> fn(&'b mut C::Leaf) -> &'b mut M,
}

impl<'a, C, A, M> Deref for BranchMutMappedMut<'a, C, A, M>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    type Target = M;

    fn deref(&self) -> &M {
        todo!(
            "FIXME? Due to limitations in rust generics over mutable or shared
references, please use map_leaf when a immutable borrow is needed"
        )
    }
}

impl<'a, C, A, M> DerefMut for BranchMutMappedMut<'a, C, A, M>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    fn deref_mut(&mut self) -> &mut M {
        (self.closure)(&mut *self.inner)
    }
}
