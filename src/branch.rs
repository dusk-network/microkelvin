// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::marker::PhantomData;
use std::ops::Deref;

use canonical::Store;

use crate::annotation::Annotated;
use crate::compound::{Child, Compound};

type Offset = usize;

pub enum Level<'a, C, S>
where
    C: Clone,
{
    #[allow(unused)]
    Borrowed(&'a C),
    #[allow(unused)]
    Owned(C, PhantomData<S>),
}

pub struct PartialBranch<'a, C, S>(Levels<'a, C, S>)
where
    C: Clone;

pub struct Levels<'a, C, S>(Vec<(Offset, Level<'a, C, S>)>)
where
    C: Clone;

impl<'a, C, S> Levels<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    pub fn new(first: Level<'a, C, S>) -> Self {
        Levels(vec![(0, first)])
    }

    pub fn depth(&self) -> usize {
        self.0.len()
    }

    pub fn top(&self) -> &(Offset, Level<'a, C, S>) {
        self.0.last().expect("always > 0 len")
    }

    pub fn top_mut(&mut self) -> &mut (Offset, Level<'a, C, S>) {
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
        self.0.push((0, Level::Owned(node, PhantomData)))
    }

    pub fn leaf(&self) -> Option<&C::Leaf> {
        let (ofs, level) = self.top();
        match level {
            Level::Borrowed(c) => match c.child(*ofs) {
                Child::Leaf(l) => Some(l),
                _ => None,
            },
            Level::Owned(c, _) => match c.child(*ofs) {
                Child::Leaf(l) => Some(l),
                _ => None,
            },
        }
    }
}

impl<'a, C, S> PartialBranch<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    fn new(root: &'a C) -> Self {
        let levels = Levels::new(Level::Borrowed(root));
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

            let (ofs, node) = match self.0.top_mut() {
                (ofs, Level::Borrowed(c)) => (ofs, *c),
                (ofs, Level::Owned(c, _)) => (ofs, &*c),
            };

            match match node.child(*ofs) {
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
                    let (ref mut ofs, _) = self.0.top_mut();
                    *ofs += 1;
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

impl<'a, C, S> Branch<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    pub fn depth(&self) -> usize {
        self.0.depth()
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

pub struct Branch<'a, C, S>(PartialBranch<'a, C, S>)
where
    C: Clone;

impl<'a, C, S> Deref for Branch<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    type Target = C::Leaf;

    fn deref(&self) -> &Self::Target {
        self.0.leaf().expect("Invalid branch")
    }
}
