// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::marker::PhantomData;
use core::mem;
use core::ops::{Deref, DerefMut};

use alloc::vec::Vec;

use canonical::{Canon, CanonError};

use crate::annotations::{Annotated, Annotation};
use crate::compound::{Child, ChildMut, Compound};

/// The argument given to a closure to `walk` a `BranchMut`.
pub enum WalkMut<'a, C, A>
where
    C: Compound,
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
    C: Compound,
{
    /// The correct leaf was found!
    Found(&'a mut C::Leaf),
    /// Step to the next child on this level
    Next,
    /// Traverse the branch deeper
    Into(&'a mut Annotated<C, A>),
}

enum LevelInnerMut<'a, C, A> {
    Borrowed(&'a mut C),
    Owned(C, PhantomData<A>),
}

/// Represents a level in the branch.
///
/// The offset is pointing at the child of the node stored behind the LevelInner
/// pointer.
pub struct LevelMut<'a, C, A> {
    offset: usize,
    inner: LevelInnerMut<'a, C, A>,
}

impl<'a, C, A> LevelMut<'a, C, A>
where
    C: Compound,
{
    /// Returns the offset of the branch level
    pub fn offset(&self) -> usize {
        self.offset
    }

    fn new_owned(node: C) -> Self {
        LevelMut {
            offset: 0,
            inner: LevelInnerMut::Owned(node, PhantomData),
        }
    }

    fn new_borrowed(node: &'a mut C) -> Self {
        LevelMut {
            offset: 0,
            inner: LevelInnerMut::Borrowed(node),
        }
    }

    fn level_child_mut(&mut self) -> ChildMut<C, A> {
        let ofs = self.offset();
        match self.inner {
            LevelInnerMut::Borrowed(ref mut n) => n.child_mut(ofs),
            LevelInnerMut::Owned(ref mut n, PhantomData) => n.child_mut(ofs),
        }
    }

    fn inner(&self) -> &LevelInnerMut<'a, C, A> {
        &self.inner
    }

    fn offset_mut(&mut self) -> &mut usize {
        &mut self.offset
    }
}

impl<'a, C, A> Deref for LevelMut<'a, C, A> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        match &self.inner {
            LevelInnerMut::Borrowed(b) => b,
            LevelInnerMut::Owned(c, _) => &c,
        }
    }
}

pub struct PartialBranchMut<'a, C, A>(LevelsMut<'a, C, A>);

pub struct LevelsMut<'a, C, A>(Vec<LevelMut<'a, C, A>>);

impl<'a, C, A> LevelsMut<'a, C, A>
where
    C: Compound + Canon,
    A: Annotation<C>,
{
    pub fn new(first: LevelMut<'a, C, A>) -> Self {
        let mut levels = vec![];
        levels.push(first);
        LevelsMut(levels)
    }

    pub fn depth(&self) -> usize {
        self.0.len()
    }

    pub fn top(&self) -> &LevelMut<'a, C, A> {
        self.0.last().expect("always > 0 len")
    }

    pub fn top_mut(&mut self) -> &mut LevelMut<'a, C, A> {
        self.0.last_mut().expect("always > 0 len")
    }

    fn advance(&mut self) {
        *self.top_mut().offset_mut() += 1
    }

    pub fn pop(&mut self) -> Option<()> {
        if self.0.len() > 1 {
            let popped_node = self.0.pop().expect("length > 1");
            let top_node = self.top_mut();
            if let ChildMut::Node(top_child) = top_node.level_child_mut() {
                *top_child = Annotated::new(popped_node.clone());
            } else {
                unreachable!("Invalid parent structure")
            }
            Some(())
        } else {
            None
        }
    }

    pub fn push(&mut self, node: C) {
        self.0.push(LevelMut::new_owned(node))
    }

    pub fn leaf(&self) -> Option<&C::Leaf> {
        let top_level = self.top();
        match top_level.inner() {
            LevelInnerMut::Borrowed(c) => {
                match c.child::<A>(top_level.offset()) {
                    Child::Leaf(l) => Some(l),
                    _ => None,
                }
            }
            LevelInnerMut::Owned(c, _) => {
                match c.child::<A>(top_level.offset()) {
                    Child::Leaf(l) => Some(l),
                    _ => None,
                }
            }
        }
    }

    fn leaf_mut(&mut self) -> Option<&mut C::Leaf> {
        let top_level = self.top_mut();
        match top_level.level_child_mut() {
            ChildMut::Leaf(l) => Some(l),
            _ => None,
        }
    }
}

impl<'a, C, A> PartialBranchMut<'a, C, A>
where
    C: Compound + Canon,
    A: Annotation<C>,
{
    fn new(root: &'a mut C) -> Self {
        let levels = LevelsMut::new(LevelMut::new_borrowed(root));
        PartialBranchMut(levels)
    }

    pub fn depth(&self) -> usize {
        self.0.depth()
    }

    fn leaf(&self) -> Option<&C::Leaf> {
        self.0.leaf()
    }

    fn leaf_mut(&mut self) -> Option<&mut C::Leaf> {
        self.0.leaf_mut()
    }

    pub fn pop(&mut self) -> Option<()> {
        self.0.pop()
    }

    fn advance(&mut self) {
        self.0.advance()
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
                State::Push(push) => self.0.push(push),
                State::Pop => match self.0.pop() {
                    Some(_) => {
                        self.advance();
                    }
                    None => return Ok(None),
                },
                State::Advance => self.advance(),
            }

            let top_node = self.0.top_mut();
            match match top_node.level_child_mut() {
                ChildMut::Leaf(l) => walker(WalkMut::Leaf(l)),
                ChildMut::Node(c) => walker(WalkMut::Node(c)),
                ChildMut::Empty => todo!(),
                ChildMut::EndOfNode => {
                    state = State::Pop;
                    continue;
                }
            } {
                StepMut::Found(_) => {
                    return Ok(Some(()));
                }
                StepMut::Next => {
                    state = State::Advance;
                }
                StepMut::Into(n) => {
                    state = State::Push(n.val()?.clone());
                }
            };
        }
    }

    fn path<P>(&mut self, mut path: P) -> Result<Option<()>, CanonError>
    where
        P: FnMut() -> usize,
    {
        let mut push = None;
        loop {
            if let Some(push) = push.take() {
                self.0.push(push);
            }

            let top_level = self.0.top_mut();

            let ofs = path();
            *top_level.offset_mut() = ofs;

            match top_level.child::<A>(ofs) {
                Child::Leaf(_) => {
                    return Ok(Some(()));
                }
                Child::Node(c) => push = Some(c.val()?.clone()),
                Child::Empty => {
                    return Ok(None);
                }
                Child::EndOfNode => {
                    return Ok(None);
                }
            }
        }
    }
}

impl<'a, C, A> Drop for BranchMut<'a, C, A>
where
    C: Compound + Canon,
    A: Annotation<C>,
{
    fn drop(&mut self) {
        // unwind when dropping
        while self.0.pop().is_some() {}
    }
}

impl<'a, C, A> BranchMut<'a, C, A>
where
    C: Compound + Canon,
    A: Annotation<C>,
{
    /// Returns the depth of the branch
    pub fn depth(&self) -> usize {
        self.0.depth()
    }

    /// Returns a reference to the levels in the branch
    pub fn levels(&self) -> &[LevelMut<C, A>] {
        &((self.0).0).0[..]
    }

    /// Performs a tree walk, returning either a valid branch or None if the
    /// walk failed.
    pub fn walk<W: FnMut(WalkMut<C, A>) -> StepMut<C, A>>(
        root: &'a mut C,
        walker: W,
    ) -> Result<Option<Self>, CanonError> {
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
    C: Compound + Canon,
    A: Annotation<C>;

impl<'a, C, A> Deref for BranchMut<'a, C, A>
where
    C: Compound + Canon,
    A: Annotation<C>,
{
    type Target = C::Leaf;

    fn deref(&self) -> &Self::Target {
        self.0.leaf().expect("Invalid branch")
    }
}

impl<'a, C, A> DerefMut for BranchMut<'a, C, A>
where
    C: Compound + Canon,
    A: Annotation<C>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.leaf_mut().expect("Invalid branch")
    }
}
