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

type Offset = usize;

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

pub enum LevelMut<'a, C, S>
where
    C: Clone,
{
    #[allow(unused)]
    Borrowed(&'a mut C),
    #[allow(unused)]
    Owned(C, PhantomData<S>),
}

impl<'a, C, S> Deref for LevelMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        match self {
            LevelMut::Borrowed(b) => b,
            LevelMut::Owned(c, _) => &c,
        }
    }
}

impl<'a, C, S> DerefMut for LevelMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            LevelMut::Borrowed(b) => b,
            LevelMut::Owned(ref mut c, _) => c,
        }
    }
}

pub struct PartialBranchMut<'a, C, S>(LevelsMut<'a, C, S>)
where
    C: Compound<S>,
    S: Store;

pub struct LevelsMut<'a, C, S>(Vec<(Offset, LevelMut<'a, C, S>)>)
where
    C: Compound<S>,
    S: Store;

impl<'a, C, S> LevelsMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    pub fn new(first: LevelMut<'a, C, S>) -> Self {
        LevelsMut(vec![(0, first)])
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn top(&self) -> &(Offset, LevelMut<'a, C, S>) {
        self.0.last().expect("always > 0 len")
    }

    pub fn top_mut(&mut self) -> &mut (Offset, LevelMut<'a, C, S>) {
        self.0.last_mut().expect("always > 0 len")
    }

    fn advance(&mut self) {
        self.top_mut().0 += 1
    }

    pub fn pop(&mut self) -> Option<()> {
        if self.0.len() > 1 {
            let (_, popped_node) = self.0.pop().expect("length > 1");
            let (ofs, top_node) = self.top_mut();
            if let ChildMut::Node(top_child) = top_node.child_mut(*ofs) {
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
        self.0.push((0, LevelMut::Owned(node, PhantomData)))
    }

    pub fn leaf(&self) -> Option<&C::Leaf> {
        let (ofs, level) = self.top();
        match level {
            LevelMut::Borrowed(c) => match c.child(*ofs) {
                Child::Leaf(l) => Some(l),
                _ => None,
            },
            LevelMut::Owned(c, _) => match c.child(*ofs) {
                Child::Leaf(l) => Some(l),
                _ => None,
            },
        }
    }

    fn leaf_mut(&mut self) -> Option<&mut C::Leaf> {
        let (ofs, level) = self.top_mut();
        match level {
            LevelMut::Borrowed(c) => match c.child_mut(*ofs) {
                ChildMut::Leaf(l) => Some(l),
                _ => None,
            },
            LevelMut::Owned(c, _) => match c.child_mut(*ofs) {
                ChildMut::Leaf(l) => Some(l),
                _ => None,
            },
        }
    }
}

impl<'a, C, S> PartialBranchMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    fn new(root: &'a mut C) -> Self {
        let levels = LevelsMut::new(LevelMut::Borrowed(root));
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

            let (ofs, node) = self.0.top_mut();

            match match node.child_mut(*ofs) {
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
