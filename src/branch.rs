// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::ops::Deref;

use alloc::vec::Vec;

use canonical::CanonError;

use crate::annotations::{AnnRef, Annotation};
use crate::compound::{Child, Compound};
use crate::walk::{Step, Walk};

#[derive(Debug)]
enum LevelNode<'a, C, A> {
    Root(&'a C),
    Val(AnnRef<'a, C, A>),
}

#[derive(Debug)]
struct Level<'a, C, A> {
    offset: usize,
    node: LevelNode<'a, C, A>,
}

impl<'a, C, A> Deref for Level<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
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

    pub fn new_val(ann: AnnRef<'a, C, A>) -> Level<'a, C, A> {
        Level {
            offset: 0,
            node: LevelNode::Val(ann),
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
    A: Annotation<C::Leaf>,
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
    A: Annotation<C::Leaf>,
{
    fn new(root: &'a C) -> Self {
        PartialBranch(vec![Level::new_root(root)])
    }

    pub fn depth(&self) -> usize {
        self.0.len()
    }

    fn leaf(&self) -> Option<&C::Leaf> {
        let top = self.top();
        let ofs = top.offset();

        match top.child(ofs) {
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

    fn walk<W>(&mut self, mut walker: W) -> Result<Option<()>, CanonError>
    where
        W: FnMut(Walk<C, A>) -> Step,
    {
        enum State<'a, C, A> {
            Init,
            Push(Level<'a, C, A>),
            Pop,
            Advance,
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
                State::Advance => self.advance(),
            }

            let top = self.top();
            let ofs = top.offset();
            let top_child = top.child(ofs);

            let step = match top_child {
                Child::Leaf(l) => walker(Walk::Leaf(l)),
                Child::Node(n) => walker(Walk::Ann(n.annotation())),
                Child::Empty => {
                    state = State::Advance;
                    continue;
                }
                Child::EndOfNode => {
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
                    if let Child::Node(n) = top_child {
                        let level: Level<'_, C, A> = Level::new_val(n.val()?);
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
                Step::Abort => {
                    return Ok(None);
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

            let ofs = path();
            let top = self.top_mut();
            *top.offset_mut() = ofs;

            match top.child(ofs) {
                Child::Leaf(_) => {
                    return Ok(Some(()));
                }
                Child::Node(n) => {
                    let level: Level<'_, C, A> = Level::new_val(n.val()?);
                    // Extend the lifetime of the Level.
                    // See comment in `Branch::walk` for justification.
                    let extended: Level<'a, C, A> =
                        unsafe { core::mem::transmute(level) };
                    push = Some(extended);
                }
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

impl<'a, C, A> Branch<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
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
    ) -> MappedBranch<'a, C, A, M> {
        MappedBranch {
            inner: self,
            closure,
        }
    }

    /// Performs a tree walk, returning either a valid branch or None if the
    /// walk failed.
    pub fn walk<W>(root: &'a C, walker: W) -> Result<Option<Self>, CanonError>
    where
        W: FnMut(Walk<C, A>) -> Step,
    {
        let mut partial = PartialBranch::new(root);
        Ok(partial.walk(walker)?.map(|()| Branch(partial)))
    }

    /// Construct a branch given a function returning child offsets
    pub fn path<P>(root: &'a C, path: P) -> Result<Option<Self>, CanonError>
    where
        P: FnMut() -> usize,
    {
        let mut partial = PartialBranch::new(root);
        Ok(partial.path(path)?.map(|()| Branch(partial)))
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
    A: Annotation<C::Leaf>,
{
    type Target = C::Leaf;

    fn deref(&self) -> &Self::Target {
        self.0.leaf().expect("Invalid branch")
    }
}

pub struct MappedBranch<'a, C, A, M>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    inner: Branch<'a, C, A>,
    closure: for<'b> fn(&'b C::Leaf) -> &'b M,
}

impl<'a, C, A, M> Deref for MappedBranch<'a, C, A, M>
where
    C: Compound<A>,
    C::Leaf: 'a,
    A: Annotation<C::Leaf>,
{
    type Target = M;

    fn deref(&self) -> &M {
        (self.closure)(&*self.inner)
    }
}

struct BranchIterator<'a, C, A, W> {
    branch: PartialBranch<'a, C, A>,
    walker: W,
}

// iterators
impl<'a, C, A, W> Iterator for BranchIterator<'a, C, A, W>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
    W: FnMut(Walk<C, A>) -> Step,
{
    type Item = Result<&'a C::Leaf, CanonError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.branch.walk(self.walker).transpose()
    }
}
