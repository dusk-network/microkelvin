// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::ops::Deref;

use alloc::rc::Rc;
use alloc::vec::Vec;

use canonical::CanonError;

use crate::annotations::{Annotated, Annotation};
use crate::compound::{Child, Compound};

#[derive(Debug)]
struct Level<A, C> {
    offset: usize,
    node: Annotated<A, C>,
}

impl<'a, C, A> Level<C, A> {
    /// Returns the offset of the branch level
    pub fn offset(&self) -> usize {
        self.offset
    }

    fn new(node: Annotated<C, A>) -> Self {
        Level { offset: 0, node }
    }

    fn offset_mut(&mut self) -> &mut usize {
        &mut self.offset
    }

    fn node(&self) -> &Annotated<C, A> {
        &self.node
    }
}

#[derive(Debug)]
pub struct PartialBranch<'a, C, A> {
    root: &'a C,
    root_offset: usize,
    levels: Vec<Level<C, A>>,
}

enum TopNode<'a, C> {
    Root(&'a C),
    Rc(Rc<C>),
}

impl<'a, C> Deref for TopNode<'a, C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        match self {
            TopNode::Root(target) => target,
            TopNode::Rc(rc) => &*rc,
        }
    }
}

impl<'a, C, A> PartialBranch<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    fn new(root: &'a C) -> Self {
        PartialBranch {
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
                Child::Leaf(ref leaf) => Some(leaf),
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

    fn walk<W>(&mut self, mut walker: W) -> Result<Option<()>, CanonError>
    where
        W: FnMut(Walk<C, A>) -> Step<C, A>,
    {
        let mut push = None;
        loop {
            if let Some(push) = push.take() {
                self.levels.push(push);
            }

            let (node, ofs) = match self.levels.last() {
                Some(top_level) => {
                    let ofs = top_level.offset();
                    (TopNode::Rc(top_level.node().val()?), ofs)
                }
                None => (TopNode::Root(self.root), self.root_offset),
            };

            let step = match node.child(ofs) {
                Child::Leaf(l) => walker(Walk::Leaf(l)),
                Child::Node(c) => walker(Walk::Node(c)),
                Child::Empty => todo!(),
                Child::EndOfNode => {
                    if self.levels.pop().is_none() {
                        // last level
                        return Ok(None);
                    } else {
                        Step::Next
                    }
                }
            };

            match step {
                Step::Found(_) => {
                    return Ok(Some(()));
                }
                Step::Next => self.advance(),
                Step::Abort => return Ok(None),
                Step::Into(n) => {
                    push = Some(Level::new(n.clone()));
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
                        Child::Node(c) => push = Some(Level::new(c.clone())),
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
    A: Annotation<C::Leaf>,
{
    /// Returns the depth of the branch
    pub fn depth(&self) -> usize {
        self.0.depth()
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
    A: Annotation<C::Leaf>,
{
    type Target = C::Leaf;

    fn deref(&self) -> &Self::Target {
        self.0.leaf().expect("Invalid branch")
    }
}
