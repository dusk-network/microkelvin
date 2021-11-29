// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::marker::PhantomData;

use alloc::vec::Vec;
use rkyv::Archive;

use crate::compound::{ArchivedChild, ArchivedCompound, Child, Compound};
use crate::walk::{All, Discriminant, Step, Walkable, Walker};
use crate::wrappers::{MaybeArchived, MaybeStored};
use crate::{ARef, Annotation, Store};

pub struct Level<'a, C, A, S>
where
    S: Store,
    C: Archive,
{
    offset: usize,
    // pub to be accesible from `walk.rs`
    pub(crate) node: MaybeArchived<'a, C>,
    _marker: PhantomData<(A, S)>,
}

impl<'a, C, A, S> Level<'a, C, A, S>
where
    S: Store,
    C: Archive + Compound<A, S>,
{
    pub fn new(root: MaybeArchived<'a, C>) -> Level<'a, C, A, S> {
        Level {
            offset: 0,
            node: root,
            _marker: PhantomData,
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

impl<'a, C, A, S> Walkable<C, A, S> for &'a Level<'a, C, A, S>
where
    S: Store,
    C: Compound<A, S>,
    C::Archived: ArchivedCompound<C, A, S>,
    A: Annotation<C::Leaf>,
{
    fn probe(&self, ofs: usize) -> Discriminant<C::Leaf, A> {
        match self.node {
            MaybeArchived::Memory(m) => match m.child(self.offset + ofs) {
                Child::Leaf(leaf) => {
                    Discriminant::Leaf(MaybeArchived::Memory(leaf))
                }
                Child::Link(link) => {
                    Discriminant::Annotation(link.annotation())
                }
                Child::Empty => Discriminant::Empty,
                Child::End => Discriminant::End,
            },
            MaybeArchived::Archived(a) => match a.child(self.offset + ofs) {
                ArchivedChild::Leaf(leaf) => {
                    Discriminant::Leaf(MaybeArchived::Archived(leaf))
                }
                ArchivedChild::Link(link) => {
                    Discriminant::Annotation(ARef::Borrowed(link.annotation()))
                }
                ArchivedChild::Empty => Discriminant::Empty,
                ArchivedChild::End => Discriminant::End,
            },
        }
    }
}

pub struct PartialBranch<'a, C, A, S>
where
    S: Store,
    C: Archive,
{
    levels: Vec<Level<'a, C, A, S>>,
    store: Option<S>,
}

impl<'a, C, A, S> PartialBranch<'a, C, A, S>
where
    S: Store,
    C: Archive + Compound<A, S>,
    C::Archived: ArchivedCompound<C, A, S>,
    C::Leaf: Archive,
{
    fn new(root: MaybeArchived<'a, C>) -> Self {
        PartialBranch {
            levels: vec![Level::new(root)],
            store: None,
        }
    }

    fn new_with_store(root: MaybeArchived<'a, C>, store: S) -> Self {
        PartialBranch {
            levels: vec![Level::new(root)],
            store: Some(store),
        }
    }

    pub fn depth(&self) -> usize {
        self.levels.len()
    }

    pub fn levels(&self) -> &[Level<C, A, S>] {
        &self.levels
    }

    fn leaf(&self) -> Option<MaybeArchived<C::Leaf>>
    where
        C: Compound<A, S>,
        C::Leaf: Archive,
    {
        let top = self.top();
        let ofs = top.offset();

        match &top.node {
            MaybeArchived::Memory(root) => match root.child(ofs) {
                Child::Leaf(l) => Some(MaybeArchived::Memory(l)),
                _ => None,
            },
            MaybeArchived::Archived(s) => match s.child(ofs) {
                ArchivedChild::Leaf(l) => Some(MaybeArchived::Archived(l)),
                _ => None,
            },
        }
    }

    fn top(&self) -> &Level<C, A, S> {
        self.levels.last().expect("Never empty")
    }

    fn top_mut(&mut self) -> &mut Level<'a, C, A, S> {
        self.levels.last_mut().expect("Never empty")
    }

    fn advance(&mut self) {
        *self.top_mut().offset_mut() += 1;
    }

    fn pop(&mut self) -> Option<Level<'a, C, A, S>> {
        // We never pop the root
        if self.levels.len() > 1 {
            self.levels.pop()
        } else {
            None
        }
    }

    fn walk<W>(&mut self, walker: &mut W) -> Option<()>
    where
        W: Walker<C, A, S>,
        C: Compound<A, S>,
        A: Annotation<C::Leaf>,
    {
        enum State<'a, C, A, S>
        where
            S: Store,
            C: Archive + Compound<A, S>,
            C::Archived: ArchivedCompound<C, A, S>,
            C::Leaf: Archive,
        {
            Init,
            Push(Level<'a, C, A, S>),
            Pop,
        }

        let mut state = State::Init;
        loop {
            match core::mem::replace(&mut state, State::Init) {
                State::Init => (),
                State::Push(push) => self.levels.push(push),
                State::Pop => match self.pop() {
                    Some(_) => {
                        self.advance();
                    }
                    None => return None,
                },
            }

            let top = self.top_mut();

            match walker.walk(&*top) {
                Step::Found(walk_ofs) => {
                    *top.offset_mut() += walk_ofs;
                    let ofs = top.offset();

                    let archived = match &top.node {
                        MaybeArchived::Memory(root) => match root.child(ofs) {
                            Child::Leaf(_) => return Some(()),
                            Child::Link(link) => match link.inner() {
                                MaybeStored::Memory(c) => {
                                    let level = Level::<C, A, S>::new(
                                        MaybeArchived::Memory(c),
                                    );
                                    let extended: Level<'a, C, A, S> =
                                        unsafe { core::mem::transmute(level) };
                                    state = State::Push(extended);
                                    continue;
                                }
                                MaybeStored::Stored(stored) => {
                                    if self.store.is_none() {
                                        self.store =
                                            Some(stored.store().clone())
                                    }
                                    stored.inner()
                                }
                            },
                            Child::Empty => return None,
                            Child::End => return None,
                        },
                        MaybeArchived::Archived(archived) => {
                            match archived.child(ofs) {
                                ArchivedChild::Leaf(_) => return Some(()),
                                ArchivedChild::Link(link) => {
                                    if let Some(ref store) = self.store {
                                        store.get_raw(link.ident())
                                    } else {
                                        unreachable!()
                                    }
                                }
                                _ => panic!("Invalid child found"),
                            }
                        }
                    };

                    // continued archived branch

                    let level: Level<C, A, S> =
                        Level::new(MaybeArchived::Archived(archived));
                    let extended: Level<'a, C, A, S> =
                        unsafe { core::mem::transmute(level) };
                    state = State::Push(extended);
                }
                Step::Advance => state = State::Pop,
                Step::Abort => {
                    return None;
                }
            }
        }
    }
}

impl<'a, C, A, S> Branch<'a, C, A, S>
where
    S: Store,
    C: Archive + Compound<A, S>,
    C::Archived: ArchivedCompound<C, A, S>,
    C::Leaf: Archive,
{
    /// Returns the depth of the branch
    pub fn depth(&self) -> usize {
        self.0.depth()
    }

    /// Returns a slice into the levels of the tree.
    pub fn levels(&self) -> &[Level<C, A, S>] {
        self.0.levels()
    }

    /// Returns a branch that maps the leaf to a specific value.
    /// Used in maps for example, to get easy access to the value of the KV-pair
    pub fn map_leaf<M>(
        self,
        closure: for<'b> fn(MaybeArchived<'b, C::Leaf>) -> &'b M,
    ) -> MappedBranch<'a, S, C, A, M>
    where
        C: Compound<A, S>,
        M: Archive,
    {
        MappedBranch {
            inner: self,
            closure,
        }
    }

    /// Returns a reference to the currently pointed-at leaf
    pub fn leaf(&self) -> MaybeArchived<C::Leaf> {
        self.0.leaf().expect("Invalid branch")
    }

    /// Performs a tree walk, returning either a valid branch or None if the
    /// walk failed.
    pub fn walk<W>(root: MaybeArchived<'a, C>, mut walker: W) -> Option<Self>
    where
        C: Compound<A, S>,
        W: Walker<C, A, S>,
        A: Annotation<C::Leaf>,
    {
        let mut partial = PartialBranch::new(root);
        partial.walk(&mut walker).map(|()| Branch(partial))
    }

    /// Performs a tree walk, returning either a valid branch or None if the
    /// walk failed.
    pub fn walk_with_store<W>(
        root: MaybeArchived<'a, C>,
        mut walker: W,
        store: S,
    ) -> Option<Self>
    where
        C: Compound<A, S>,
        W: Walker<C, A, S>,
        A: Annotation<C::Leaf>,
    {
        let mut partial = PartialBranch::new_with_store(root, store);
        partial.walk(&mut walker).map(|()| Branch(partial))
    }
}

/// Reprents an immutable branch view into a collection.
///
/// Branche are always guaranteed to point at a leaf, and can be dereferenced
/// to the pointed-at leaf.
pub struct Branch<'a, C, A, S>(PartialBranch<'a, C, A, S>)
where
    S: Store,
    C: Archive;

/// A branch that applies a map to its leaf
pub struct MappedBranch<'a, S, C, A, M>
where
    S: Store,
    C: Compound<A, S>,
    C::Leaf: Archive,
{
    inner: Branch<'a, C, A, S>,
    closure: for<'b> fn(MaybeArchived<'b, C::Leaf>) -> &'b M,
}

impl<'a, S, C, A, M> MappedBranch<'a, S, C, A, M>
where
    S: Store,
    C: Compound<A, S>,
    C::Archived: ArchivedCompound<C, A, S>,
    C::Leaf: Archive,
    M: Archive,
{
    /// Get the mapped leaf of the branch
    pub fn leaf(&'a self) -> &'a M {
        (self.closure)(self.inner.leaf())
    }
}

pub enum BranchIterator<'a, S, C, A, W>
where
    S: Store,
    C: Archive,
{
    Initial(Branch<'a, C, A, S>, W),
    Intermediate(Branch<'a, C, A, S>, W),
    Exhausted,
}

// iterators
impl<'a, C, A, S> IntoIterator for Branch<'a, C, A, S>
where
    S: Store,
    C: Compound<A, S>,
    C::Leaf: 'a + Archive,
    C::Archived: ArchivedCompound<C, A, S>,
    A: Annotation<C::Leaf>,
{
    type Item = MaybeArchived<'a, C::Leaf>;

    type IntoIter = BranchIterator<'a, S, C, A, All>;

    fn into_iter(self) -> Self::IntoIter {
        BranchIterator::Initial(self, All)
    }
}

impl<'a, S, C, A, W> Iterator for BranchIterator<'a, S, C, A, W>
where
    S: Store,
    C: Compound<A, S>,
    C::Leaf: 'a + Archive,
    C::Archived: ArchivedCompound<C, A, S>,
    A: Annotation<C::Leaf>,
    W: Walker<C, A, S>,
{
    type Item = MaybeArchived<'a, C::Leaf>;

    fn next(&mut self) -> Option<Self::Item> {
        match core::mem::replace(self, BranchIterator::Exhausted) {
            BranchIterator::Initial(branch, walker) => {
                *self = BranchIterator::Intermediate(branch, walker);
            }
            BranchIterator::Intermediate(mut branch, mut walker) => {
                branch.0.advance();
                // access partialbranch
                match branch.0.walk(&mut walker) {
                    None => {
                        *self = BranchIterator::Exhausted;
                        return None;
                    }
                    Some(_) => {
                        *self = BranchIterator::Intermediate(branch, walker);
                    }
                }
            }
            BranchIterator::Exhausted => {
                return None;
            }
        }

        match self {
            BranchIterator::Intermediate(branch, _) => {
                let leaf = branch.leaf();
                let leaf_extended: MaybeArchived<'a, C::Leaf> =
                    unsafe { core::mem::transmute(leaf) };
                Some(leaf_extended)
            }
            _ => unreachable!(),
        }
    }
}

pub enum MappedBranchIterator<'a, S, C, A, M, W>
where
    S: Store,
    C: Archive + Compound<A, S>,
    C::Leaf: Archive,
{
    Initial(MappedBranch<'a, S, C, A, M>, W),
    Intermediate(MappedBranch<'a, S, C, A, M>, W),
    Exhausted,
}

impl<'a, S, C, A, M> IntoIterator for MappedBranch<'a, S, C, A, M>
where
    S: Store,
    C: Compound<A, S>,
    C::Archived: ArchivedCompound<C, A, S>,
    C::Leaf: Archive,
    A: Annotation<C::Leaf>,
    M: 'a + Archive,
{
    type Item = &'a M;

    type IntoIter = MappedBranchIterator<'a, S, C, A, M, All>;

    fn into_iter(self) -> Self::IntoIter {
        MappedBranchIterator::Initial(self, All)
    }
}

impl<'a, S, C, A, M, W> Iterator for MappedBranchIterator<'a, S, C, A, M, W>
where
    S: Store,
    C: Compound<A, S>,
    C::Archived: ArchivedCompound<C, A, S>,
    A: Annotation<C::Leaf>,
    M: 'a + Archive,
    W: Walker<C, A, S>,
{
    type Item = &'a M;

    fn next(&mut self) -> Option<Self::Item> {
        match core::mem::replace(self, Self::Exhausted) {
            Self::Initial(branch, walker) => {
                *self = Self::Intermediate(branch, walker);
            }
            Self::Intermediate(mut branch, mut walker) => {
                branch.inner.0.advance();
                // access partialbranch
                match branch.inner.0.walk(&mut walker) {
                    None => {
                        *self = Self::Exhausted;
                        return None;
                    }
                    Some(_) => {
                        *self = Self::Intermediate(branch, walker);
                    }
                }
            }
            Self::Exhausted => {
                return None;
            }
        }

        match self {
            Self::Intermediate(branch, _) => {
                let leaf = branch.leaf();
                let leaf_extended: &'a M =
                    unsafe { core::mem::transmute(leaf) };
                Some(leaf_extended)
            }
            _ => unreachable!(),
        }
    }
}
