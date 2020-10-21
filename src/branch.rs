// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::marker::PhantomData;
use core::ops::Deref;

use canonical::Store;

use crate::annotation::Annotated;
use crate::compound::{Child, Compound};

use const_arrayvec::ArrayVec;

pub struct Level<'a, C, S>
where
    C: Clone,
{
    offset: usize,
    inner: LevelInner<'a, C, S>,
}

impl<'a, C, S> Deref for Level<'a, C, S>
where
    C: Clone,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        match self.inner {
            LevelInner::Borrowed(c) => c,
            LevelInner::Owned(ref c, _) => c,
        }
    }
}

impl<'a, C, S> Level<'a, C, S>
where
    C: Clone,
{
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

    pub fn offset(&self) -> usize {
        self.offset
    }

    fn offset_mut(&mut self) -> &mut usize {
        &mut self.offset
    }

    fn inner(&self) -> &LevelInner<'a, C, S> {
        &self.inner
    }
}

#[derive(Clone)]
enum LevelInner<'a, C, S>
where
    C: Clone,
{
    Borrowed(&'a C),
    Owned(C, PhantomData<S>),
}

pub struct PartialBranch<'a, C, S, const N: usize>(Levels<'a, C, S, N>)
where
    C: Clone;

pub struct Levels<'a, C, S, const N: usize>(ArrayVec<Level<'a, C, S>, N>)
where
    C: Clone;

impl<'a, C, S, const N: usize> Levels<'a, C, S, N>
where
    C: Compound<S>,
    S: Store,
{
    pub fn new(node: &'a C) -> Self {
        let mut levels: ArrayVec<Level<'a, C, S>, N> = ArrayVec::new();
        levels.push(Level::new_borrowed(node));
        Levels(levels)
    }

    pub fn depth(&self) -> usize {
        self.0.len()
    }

    pub fn top(&self) -> &Level<'a, C, S> {
        self.0.last().expect("always > 0 len")
    }

    pub fn top_mut(&mut self) -> &mut Level<'a, C, S> {
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

impl<'a, C, S, const N: usize> PartialBranch<'a, C, S, N>
where
    C: Compound<S>,
    S: Store,
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

    fn walk<W>(&mut self, mut walker: W) -> Result<Option<()>, S::Error>
    where
        W: FnMut(Walk<C, S>) -> Step<C, S>,
    {
        let mut push = None;
        loop {
            if let Some(push) = push.take() {
                self.0.push(push)
            }

            let top_level = self.0.top_mut();

            match match top_level.child(top_level.offset()) {
                Child::Leaf(l) => walker(Walk::Leaf(l)),
                Child::Node(c) => walker(Walk::Node(c)),
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
                Step::Into(n) => {
                    push = Some(n.val()?.clone());
                }
            }
        }
    }
}

pub enum Walk<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    Leaf(&'a C::Leaf),
    Node(&'a Annotated<C, S>),
}

pub enum Step<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    Found(&'a C::Leaf),
    Next,
    Into(&'a Annotated<C, S>),
}

impl<'a, C, S, const N: usize> Branch<'a, C, S, N>
where
    C: Compound<S>,
    S: Store,
{
    pub fn depth(&self) -> usize {
        self.0.depth()
    }

    pub fn levels(&self) -> &[Level<C, S>] {
        &((self.0).0).0[..]
    }

    pub fn walk<W: FnMut(Walk<C, S>) -> Step<C, S>>(
        root: &'a C,
        walker: W,
    ) -> Result<Option<Self>, S::Error> {
        let mut partial = PartialBranch::new(root);
        Ok(match partial.walk(walker)? {
            Some(()) => Some(Branch(partial)),
            None => None,
        })
    }
}

pub struct Branch<'a, C, S, const N: usize>(PartialBranch<'a, C, S, N>)
where
    C: Clone;

impl<'a, C, S, const N: usize> Deref for Branch<'a, C, S, N>
where
    C: Compound<S>,
    S: Store,
{
    type Target = C::Leaf;

    fn deref(&self) -> &Self::Target {
        self.0.leaf().expect("Invalid branch")
    }
}
