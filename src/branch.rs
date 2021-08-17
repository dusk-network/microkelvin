// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::ops::Deref;

use alloc::vec::Vec;

use crate::annotations::Annotation;
use crate::compound::{Child, Compound};
use crate::link::LinkCompound;
use crate::walk::{AllLeaves, Step, Walk, Walker};

#[derive(Debug)]
enum LevelNode<'a, C, A> {
    Root(&'a C),
    Val(LinkCompound<'a, C, A>),
}

#[derive(Debug)]
pub struct Level<'a, C, A> {
    offset: usize,
    node: LevelNode<'a, C, A>,
}

impl<'a, C, A> Deref for Level<'a, C, A>
where
    C: Compound<A>,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &*self.node
    }
}

impl<'a, C, A> Level<'a, C, A> {
    pub fn new_root(root: &'a C) -> Level<'a, C, A> {
        Level {
            offset: 0,
            node: LevelNode::Root(root),
        }
    }

    pub fn new_val(link_compound: LinkCompound<'a, C, A>) -> Level<'a, C, A> {
        Level {
            offset: 0,
            node: LevelNode::Val(link_compound),
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

#[derive(Debug)]
pub struct PartialBranch<'a, C, A>(Vec<Level<'a, C, A>>);

impl<'a, C, A> Deref for LevelNode<'a, C, A>
where
    C: Compound<A>,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        match self {
            LevelNode::Root(target) => target,
            LevelNode::Val(val) => &**val,
        }
    }
}

impl<'a, C, A> PartialBranch<'a, C, A>
where
    C: Compound<A>,
{
    fn new(root: &'a C) -> Self {
        PartialBranch(vec![Level::new_root(root)])
    }

    pub fn depth(&self) -> usize {
        self.0.len()
    }

    pub fn levels(&self) -> &[Level<C, A>] {
        &self.0
    }

    fn leaf(&self) -> Option<&C::Leaf> {
        let top = self.top();
        let ofs = top.offset();

        match (**top).child(ofs) {
            Child::Leaf(l) => Some(l),
            _ => None,
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

    fn walk<W>(&mut self, walker: &mut W) -> Result<Option<()>, ()>
    where
        W: Walker<C, A>,
    {
        enum State<'a, C, A> {
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
                    None => return Ok(None),
                },
            }

            let top = self.top_mut();
            let step = walker.walk(Walk::new(&**top, top.offset()));

            match step {
                Step::Found(walk_ofs) => {
                    *top.offset_mut() += walk_ofs;
                    return Ok(Some(()));
                }
                Step::Into(walk_ofs) => {
                    *top.offset_mut() += walk_ofs;
                    let ofs = top.offset();
                    let top_child = top.child(ofs);
                    if let Child::Node(n) = top_child {
                        let level: Level<'_, C, A> = Level::new_val(n.inner()?);
                        // Extend the lifetime of the Level.
                        //
                        // JUSTIFICATION
                        //
                        // The `Vec<Level<'a, C, A>>` used here cannot be
                        // expressed in safe rust, since it relies on the
                        // elements of the `Vec` refering to prior elements in
                        // the same `Vec`.
                        //
                        // This vec from the start contains one single `Level`
                        // of variant in turn containing a `LevelNode::Root(&'a
                        // C)`
                        //
                        // The first step `Into` will add a `Level` with the
                        // following reference structure
                        // `LevelNode::Val(AnnRef<'a, C, A>)` -> `Val<'a, C>` ->
                        // ReprInner (from canonical) which in turns contains
                        // the value of the next node behind an `Rc<C>`.
                        //
                        // The address of the pointed-to `C` thus remains
                        // unchanged, even if the `Vec` in `PartialBranch`
                        // re-allocates.
                        //
                        // The same is true of `LevelNode::Root` since it is a
                        // reference that just gets copied over to the new
                        // allocation.
                        //
                        // Additionally, the `Vec` is only ever changed at its
                        // end, either pushed or popped, so any reference "Up"
                        // the branch will always remain valid.
                        //
                        // Since `'a` controls the whole lifetime of the access
                        // to the tree, there is also no
                        // way for the tree to change in
                        // the meantime, thus invalidating the pointers is
                        // not possible, and this extension of the lifetime of
                        // the level is safe.

                        let extended: Level<'a, C, A> =
                            unsafe { core::mem::transmute(level) };
                        state = State::Push(extended);
                    } else {
                        panic!("Attempted descent into non-node")
                    }
                }
                Step::Advance => state = State::Pop,
                Step::Abort => {
                    return Ok(None);
                }
            }
        }
    }
}

impl<'a, C, A> Branch<'a, C, A>
where
    C: Compound<A>,
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
    ) -> MappedBranch<'a, C, A, M> {
        MappedBranch {
            inner: self,
            closure,
        }
    }

    /// Performs a tree walk, returning either a valid branch or None if the
    /// walk failed.
    pub fn walk<W>(root: &'a C, mut walker: W) -> Result<Option<Self>, ()>
    where
        W: Walker<C, A>,
    {
        let mut partial = PartialBranch::new(root);
        Ok(partial.walk(&mut walker)?.map(|()| Branch(partial)))
    }
}

/// Reprents an immutable branch view into a collection.
///
/// Branche are always guaranteed to point at a leaf, and can be dereferenced
/// to the pointed-at leaf.
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

pub struct MappedBranch<'a, C, A, M>
where
    C: Compound<A>,
{
    inner: Branch<'a, C, A>,
    closure: for<'b> fn(&'b C::Leaf) -> &'b M,
}

impl<'a, C, A, M> Deref for MappedBranch<'a, C, A, M>
where
    C: Compound<A>,
    C::Leaf: 'a,
{
    type Target = M;

    fn deref(&self) -> &M {
        (self.closure)(&*self.inner)
    }
}

pub enum BranchIterator<'a, C, A, W> {
    Initial(Branch<'a, C, A>, W),
    Intermediate(Branch<'a, C, A>, W),
    Exhausted,
}

// iterators
impl<'a, C, A> IntoIterator for Branch<'a, C, A>
where
    C: Compound<A>,
{
    type Item = Result<&'a C::Leaf, ()>;

    type IntoIter = BranchIterator<'a, C, A, AllLeaves>;

    fn into_iter(self) -> Self::IntoIter {
        BranchIterator::Initial(self, AllLeaves)
    }
}

impl<'a, C, A, W> Iterator for BranchIterator<'a, C, A, W>
where
    C: Compound<A>,
    W: Walker<C, A>,
{
    type Item = Result<&'a C::Leaf, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        match core::mem::replace(self, BranchIterator::Exhausted) {
            BranchIterator::Initial(branch, walker) => {
                *self = BranchIterator::Intermediate(branch, walker);
            }
            BranchIterator::Intermediate(mut branch, mut walker) => {
                branch.0.advance();
                // access partialbranch
                match branch.0.walk(&mut walker) {
                    Ok(None) => {
                        *self = BranchIterator::Exhausted;
                        return None;
                    }
                    Ok(Some(..)) => {
                        *self = BranchIterator::Intermediate(branch, walker);
                    }
                    Err(e) => {
                        return Some(Err(e));
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
                Some(Ok(leaf_extended))
            }
            _ => unreachable!(),
        }
    }
}

pub enum MappedBranchIterator<'a, C, A, W, M>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    Initial(MappedBranch<'a, C, A, M>, W),
    Intermediate(MappedBranch<'a, C, A, M>, W),
    Exhausted,
}

impl<'a, C, A, M> IntoIterator for MappedBranch<'a, C, A, M>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
    M: 'a,
{
    type Item = Result<&'a M, ()>;

    type IntoIter = MappedBranchIterator<'a, C, A, AllLeaves, M>;

    fn into_iter(self) -> Self::IntoIter {
        MappedBranchIterator::Initial(self, AllLeaves)
    }
}

impl<'a, C, A, W, M> Iterator for MappedBranchIterator<'a, C, A, W, M>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
    W: Walker<C, A>,
    M: 'a,
{
    type Item = Result<&'a M, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        match core::mem::replace(self, Self::Exhausted) {
            Self::Initial(branch, walker) => {
                *self = Self::Intermediate(branch, walker);
            }
            Self::Intermediate(mut branch, mut walker) => {
                branch.inner.0.advance();
                // access partialbranch
                match branch.inner.0.walk(&mut walker) {
                    Ok(None) => {
                        *self = Self::Exhausted;
                        return None;
                    }
                    Ok(Some(..)) => {
                        *self = Self::Intermediate(branch, walker);
                    }
                    Err(e) => {
                        return Some(Err(e));
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
                Some(Ok(leaf_extended))
            }
            _ => unreachable!(),
        }
    }
}
