// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::marker::PhantomData;
use std::mem;
use std::ops::{Deref, DerefMut};

use canonical::Store;

use crate::annotation::Annotated;
use crate::compound::{Child, ChildMut, Compound};

pub enum WalkMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    Leaf(&'a mut C::Leaf),
    Node(&'a mut Annotated<C, S>),
}

pub enum StepMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    Found(&'a mut C::Leaf),
    Next,
    Into(&'a mut Annotated<C, S>),
}

enum LevelInnerMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    Borrowed(&'a mut C),
    Owned(C, PhantomData<S>),
}

pub struct LevelMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    offset: usize,
    inner: LevelInnerMut<'a, C, S>,
}

impl<'a, C, S> LevelMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
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

    fn level_child_mut(&mut self) -> ChildMut<C, S> {
        let ofs = self.offset();
        match self.inner {
            LevelInnerMut::Borrowed(ref mut n) => n.child_mut(ofs),
            LevelInnerMut::Owned(ref mut n, _) => n.child_mut(ofs),
        }
    }

    fn inner(&self) -> &LevelInnerMut<'a, C, S> {
        &self.inner
    }

    fn offset_mut(&mut self) -> &mut usize {
        &mut self.offset
    }
}

impl<'a, C, S> Deref for LevelMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        match self.inner() {
            LevelInnerMut::Borrowed(b) => b,
            LevelInnerMut::Owned(c, _) => &c,
        }
    }
}

pub struct PartialBranchMut<'a, C, S>(LevelsMut<'a, C, S>)
where
    C: Compound<S>,
    S: Store;

pub struct LevelsMut<'a, C, S>(Vec<LevelMut<'a, C, S>>)
where
    C: Compound<S>,
    S: Store;

impl<'a, C, S> LevelsMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    pub fn new(first: LevelMut<'a, C, S>) -> Self {
        LevelsMut(vec![first])
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn top(&self) -> &LevelMut<'a, C, S> {
        self.0.last().expect("always > 0 len")
    }

    pub fn top_mut(&mut self) -> &mut LevelMut<'a, C, S> {
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
                *top_child = Annotated::new(popped_node.clone())
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
            LevelInnerMut::Borrowed(c) => match c.child(top_level.offset()) {
                Child::Leaf(l) => Some(l),
                _ => None,
            },
            LevelInnerMut::Owned(c, _) => match c.child(top_level.offset()) {
                Child::Leaf(l) => Some(l),
                _ => None,
            },
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

impl<'a, C, S> PartialBranchMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    fn new(root: &'a mut C) -> Self {
        let levels = LevelsMut::new(LevelMut::new_borrowed(root));
        PartialBranchMut(levels)
    }

    pub fn len(&self) -> usize {
        self.0.len()
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

    fn walk<W>(&mut self, mut walker: W) -> Result<Option<()>, S::Error>
    where
        W: FnMut(WalkMut<C, S>) -> StepMut<C, S>,
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
}

impl<'a, C, S> Drop for PartialBranchMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    fn drop(&mut self) {
        // unwind when dropping
        while let Some(_) = self.pop() {}
    }
}

impl<'a, C, S> BranchMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn levels(&self) -> &[LevelMut<C, S>] {
        &((self.0).0).0[..]
    }

    pub fn walk<W: FnMut(WalkMut<C, S>) -> StepMut<C, S>>(
        root: &'a mut C,
        walker: W,
    ) -> Result<Option<Self>, S::Error> {
        let mut partial = PartialBranchMut::new(root);
        Ok(match partial.walk(walker)? {
            Some(()) => Some(BranchMut(partial)),
            None => None,
        })
    }
}

pub struct BranchMut<'a, C, S>(PartialBranchMut<'a, C, S>)
where
    C: Compound<S>,
    S: Store;

impl<'a, C, S> Deref for BranchMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    type Target = C::Leaf;

    fn deref(&self) -> &Self::Target {
        self.0.leaf().expect("Invalid branch")
    }
}

impl<'a, C, S> DerefMut for BranchMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.leaf_mut().expect("Invalid branch")
    }
}
