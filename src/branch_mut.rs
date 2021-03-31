// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::mem;
use core::ops::{Deref, DerefMut};

use alloc::vec::Vec;

use canonical::CanonError;

use crate::annotations::{AnnRefMut, Annotation};
use crate::compound::{Child, ChildMut, Compound};
use crate::walk::{Step, Walk};

#[derive(Debug)]
enum LevelNodeMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    Root(&'a mut C),
    Val(AnnRefMut<'a, C, A>),
}

impl<'a, C, A> Deref for LevelNodeMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        match self {
            LevelNodeMut::Root(target) => target,
            LevelNodeMut::Val(val) => &**val,
        }
    }
}

impl<'a, C, A> DerefMut for LevelNodeMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            LevelNodeMut::Root(target) => target,
            LevelNodeMut::Val(val) => &mut **val,
        }
    }
}

#[derive(Debug)]
pub struct LevelMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    offset: usize,
    node: LevelNodeMut<'a, C, A>,
}

impl<'a, C, A> Deref for LevelMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &*self.node
    }
}

impl<'a, C, A> DerefMut for LevelMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.node
    }
}

impl<'a, C, A> LevelMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    fn new_root(root: &'a mut C) -> LevelMut<'a, C, A> {
        LevelMut {
            offset: 0,
            node: LevelNodeMut::Root(root),
        }
    }

    fn new_val(ann: AnnRefMut<'a, C, A>) -> LevelMut<'a, C, A> {
        LevelMut {
            offset: 0,
            node: LevelNodeMut::Val(ann),
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

pub struct PartialBranchMut<'a, C, A>(Vec<LevelMut<'a, C, A>>)
where
    C: Compound<A>,
    A: Annotation<C::Leaf>;

impl<'a, C, A> PartialBranchMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
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

    fn leaf(&self) -> Option<&'a C::Leaf> {
        let top = self.top();
        let ofs = top.offset();

        match top.child(ofs) {
            Child::Leaf(l) => {
                let l: &C::Leaf = l;
                let l_extended: &'a C::Leaf =
                    unsafe { core::mem::transmute(l) };
                Some(l_extended)
            }
            _ => None,
        }
    }

    fn leaf_mut(&mut self) -> Option<&'a mut C::Leaf> {
        let top = self.top_mut();
        let ofs = top.offset();

        match top.child_mut(ofs) {
            ChildMut::Leaf(l) => {
                let l: &'a mut C::Leaf = unsafe { core::mem::transmute(l) };
                Some(l)
            }
            _ => None,
        }
    }

    fn advance(&mut self) {
        *self.top_mut().offset_mut() += 1
    }

    fn pop(&mut self) -> Option<()> {
        self.0.pop().map(|_| ())
    }

    fn walk<W>(&mut self, mut walker: W) -> Result<Option<()>, CanonError>
    where
        W: FnMut(Walk<C, A>) -> Step,
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

            let top = self.top_mut();
            let ofs = top.offset();
            let top_child = top.child_mut(ofs);

            let step = match &top_child {
                ChildMut::Leaf(l) => walker(Walk::Leaf(*l)),
                ChildMut::Node(c) => walker(Walk::Ann(c.annotation())),
                ChildMut::Empty => todo!(),
                ChildMut::EndOfNode => {
                    state = State::Pop;
                    continue;
                }
            };

            match step {
                Step::Found => {
                    return Ok(Some(()));
                }
                Step::Next => {
                    state = State::Advance;
                }
                Step::Into => {
                    if let ChildMut::Node(n) = top_child {
                        let level: LevelMut<'_, C, A> =
                            LevelMut::new_val(n.val_mut()?);
                        // extend the lifetime of the Level.
                        let extended: LevelMut<'a, C, A> =
                            unsafe { core::mem::transmute(level) };
                        state = State::Push(extended);
                    } else {
                        panic!("Attempted descent into non-node")
                    }
                }

                Step::Abort => return Ok(None),
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

            let ofs = path();
            let top = self.top_mut();
            *top.offset_mut() = ofs;

            match top.child_mut(ofs) {
                ChildMut::Leaf(_) => {
                    return Ok(Some(()));
                }
                ChildMut::Node(n) => {
                    let level: LevelMut<'_, C, A> =
                        LevelMut::new_val(n.val_mut()?);
                    // extend the lifetime of the Level.
                    let extended: LevelMut<'a, C, A> =
                        unsafe { core::mem::transmute(level) };
                    push = Some(extended);
                }
                ChildMut::Empty => {
                    return Ok(None);
                }
                ChildMut::EndOfNode => {
                    return Ok(None);
                }
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
        W: FnMut(Walk<C, A>) -> Step,
    {
        let mut partial = PartialBranchMut::new(root);
        Ok(partial.walk(walker)?.map(|()| BranchMut(partial)))
    }

    /// Construct a branch given a function returning child offsets
    pub fn path<P>(root: &'a mut C, path: P) -> Result<Option<Self>, CanonError>
    where
        P: FnMut() -> usize,
    {
        let mut partial = PartialBranchMut::new(root);
        Ok(partial.path(path)?.map(|()| BranchMut(partial)))
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
