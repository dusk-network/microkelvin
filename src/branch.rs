// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::marker::PhantomData;
use core::ops::Deref;

use alloc::vec::Vec;

use canonical::CanonError;

use crate::annotations::Annotated;
use crate::compound::{Child, Compound};

/// Represents a level in the branch.
///
/// The offset is pointing at the child of the node stored behind the LevelInner
/// pointer.
#[derive(Debug)]
pub struct Level<'a, C, A> {
    offset: usize,
    inner: LevelInner<'a, C, A>,
}

impl<'a, C, A> Deref for Level<'a, C, A> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        match self.inner {
            LevelInner::Borrowed(c) => c,
            LevelInner::Owned(ref c, _) => c,
        }
    }
}

impl<'a, C, A> Level<'a, C, A> {
    /// Returns the offset of the branch level
    pub fn offset(&self) -> usize {
        self.offset
    }

    fn new_owned(node: C) -> Self {
        Level {
            offset: 0,
            inner: LevelInner::Owned(node, PhantomData),
        }
    }

    fn new_borrowed(node: &'a C) -> Self {
        Level {
            offset: 0,
            inner: LevelInner::Borrowed(node),
        }
    }

    fn offset_mut(&mut self) -> &mut usize {
        &mut self.offset
    }

    fn inner(&self) -> &LevelInner<'a, C, A> {
        &self.inner
    }
}

#[derive(Clone, Debug)]
enum LevelInner<'a, C, A> {
    Borrowed(&'a C),
    Owned(C, PhantomData<A>),
}

#[derive(Debug)]
pub struct PartialBranch<'a, C, A>(Levels<'a, C, A>);

#[derive(Debug)]
pub struct Levels<'a, C, A>(Vec<Level<'a, C, A>>);

impl<'a, C, A> Levels<'a, C, A>
where
    C: Compound<A>,
{
    pub fn new(node: &'a C) -> Self {
        let mut levels: Vec<Level<'a, C, A>> = Vec::new();
        levels.push(Level::new_borrowed(node));
        Levels(levels)
    }

    pub fn depth(&self) -> usize {
        self.0.len()
    }

    pub fn top(&self) -> &Level<'a, C, A> {
        self.0.last().expect("always > 0 len")
    }

    pub fn top_mut(&mut self) -> &mut Level<'a, C, A> {
        self.0.last_mut().expect("always > 0 len")
    }

    pub fn pop(&mut self) -> bool {
        if self.0.len() > 1 {
            self.0.pop();
            true
        } else {
            false
        }
    }

    pub fn push(&mut self, node: C) {
        self.0.push(Level::new_owned(node))
    }

    pub fn leaf(&self) -> Option<&C::Leaf> {
        let level = self.top();
        match level.inner() {
            LevelInner::Borrowed(c) => match c.child(level.offset()) {
                Child::Leaf(l) => Some(l),
                _ => None,
            },
            LevelInner::Owned(c, _) => match c.child(level.offset()) {
                Child::Leaf(l) => Some(l),
                _ => None,
            },
        }
    }
}

impl<'a, C, A> PartialBranch<'a, C, A>
where
    C: Compound<A>,
{
    fn new(root: &'a C) -> Self {
        let levels = Levels::new(root);
        PartialBranch(levels)
    }

    pub fn depth(&self) -> usize {
        self.0.depth()
    }

    fn leaf(&self) -> Option<&C::Leaf> {
        self.0.leaf()
    }

    fn walk<W>(&mut self, mut walker: W) -> Result<Option<()>, CanonError>
    where
        W: FnMut(Walk<C, A>) -> Step<C, A>,
    {
        let mut push = None;
        loop {
            if let Some(push) = push.take() {
                self.0.push(push);
            }

            let top_level = self.0.top_mut();

            match match top_level.child(top_level.offset()) {
                Child::Leaf(l) => walker(Walk::Leaf(l)),
                Child::Node(c) => walker(Walk::Node(c)),
                Child::Empty => todo!(),
                Child::EndOfNode => {
                    if !self.0.pop() {
                        // last level
                        return Ok(None);
                    } else {
                        Step::Next
                    }
                }
            } {
                Step::Found(_) => {
                    return Ok(Some(()));
                }
                Step::Next => {
                    *self.0.top_mut().offset_mut() += 1;
                }
                Step::Abort => return Ok(None),
                Step::Into(n) => {
                    push = Some(*n.val()?);
                }
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
                self.0.push(push);
            }

            let top_level = self.0.top_mut();

            let ofs = path();
            *top_level.offset_mut() = ofs;

            match top_level.child(ofs) {
                Child::Leaf(_) => {
                    return Ok(Some(()));
                }
                Child::Node(c) => push = Some(*c.val()?),
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

/// The argument given to a closure to `walk` a `Branch`.
pub enum Walk<'a, C, A>
where
    C: Compound<A>,
{
    /// Walk encountered a leaf
    Leaf(&'a C::Leaf),
    /// Walk encountered an annotated node
    Node(&'a Annotated<C, A>),
    /// Abort search
    Abort,
}

/// The return value from a closure to `walk` the tree.
///
/// Determines how the `Branch` is constructed
pub enum Step<'a, C, A>
where
    C: Compound<A>,
{
    /// The correct leaf was found!
    Found(&'a C::Leaf),
    /// Step to the next child on this level
    Next,
    /// Abort search
    Abort,
    /// Traverse the branch deeper
    Into(&'a Annotated<C, A>),
}

impl<'a, C, A> Branch<'a, C, A>
where
    C: Compound<A>,
{
    /// Returns the depth of the branch
    pub fn depth(&self) -> usize {
        self.0.depth()
    }

    /// Returns a reference to the levels in the branch
    pub fn levels(&self) -> &[Level<C, A>] {
        &((self.0).0).0[..]
    }

    /// Performs a tree walk, returning either a valid branch or None if the
    /// walk failed.
    pub fn walk<W>(root: &'a C, walker: W) -> Result<Option<Self>, CanonError>
    where
        W: FnMut(Walk<C, A>) -> Step<C, A>,
    {
        let mut partial = PartialBranch::new(root);
        Ok(match partial.walk(walker)? {
            Some(()) => Some(Branch(partial)),
            None => None,
        })
    }

    /// Construct a branch given a function returning child offsets
    pub fn path<P>(root: &'a C, path: P) -> Result<Option<Self>, CanonError>
    where
        P: FnMut() -> usize,
    {
        let mut partial = PartialBranch::new(root);
        Ok(match partial.path(path)? {
            Some(()) => Some(Branch(partial)),
            None => None,
        })
    }
}

/// Reprents an immutable branch view into a collection.
///
/// Branche are always guaranteed to point at a leaf, and can be dereferenced
/// to the pointed-at leaf.
///
/// The const generic `N` represents the maximum depth of the branch.
#[derive(Debug)]
pub struct Branch<'a, C, A>(PartialBranch<'a, C, A>);

impl<'a, C, A> Deref for Branch<'a, C, A>
where
    C: Compound<A>,
{
    type Target = C::Leaf;

    fn deref(&self) -> &Self::Target {
        self.0.leaf().expect("Invalid branch")
    }
}
