// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::marker::PhantomData;
use std::ops::Deref;

use alloc::vec::Vec;
use bytecheck::CheckBytes;
use rkyv::validation::validators::DefaultValidator;
use rkyv::Archive;

use crate::compound::{ArchivedChild, ArchivedCompound, Child, Compound};
use crate::storage::StoreRef;
use crate::tower::{WellArchived, WellFormed};
use crate::walk::{All, Discriminant, Step, Walkable, Walker};
use crate::wrappers::{MaybeArchived, MaybeStored};
use crate::{ARef, Annotation, Fundamental};

pub struct Level<'a, C, A>
where
    C: Archive,
{
    offset: usize,
    // pub to be accesible from `walk.rs`
    pub(crate) node: MaybeArchived<'a, C>,
    _marker: PhantomData<A>,
}

impl<'a, C, A> Level<'a, C, A>
where
    C: Archive + Compound<A>,
{
    pub fn new(root: MaybeArchived<'a, C>) -> Level<'a, C, A> {
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

impl<'a, C, A> Walkable<C, A> for &'a Level<'a, C, A>
where
    C: Compound<A> + WellFormed,
    C::Leaf: WellFormed,
    C::Archived: ArchivedCompound<C, A> + WellArchived<C>,
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

pub struct PartialBranch<'a, C, A>
where
    C: Archive,
{
    levels: Vec<Level<'a, C, A>>,
    store: Option<StoreRef>,
}

impl<'a, C, A> PartialBranch<'a, C, A>
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    C::Leaf: Archive,
{
    fn new(root: MaybeArchived<'a, C>) -> Self {
        PartialBranch {
            levels: vec![Level::new(root)],
            store: None,
        }
    }

    fn new_with_store(root: MaybeArchived<'a, C>, store: StoreRef) -> Self {
        PartialBranch {
            levels: vec![Level::new(root)],
            store: Some(store),
        }
    }

    pub fn depth(&self) -> usize {
        self.levels.len()
    }

    pub fn levels(&self) -> &[Level<C, A>] {
        &self.levels
    }

    fn leaf(&self) -> Option<MaybeArchived<C::Leaf>>
    where
        C: Compound<A>,
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

    fn top(&self) -> &Level<C, A> {
        self.levels.last().expect("Never empty")
    }

    fn top_mut(&mut self) -> &mut Level<'a, C, A> {
        self.levels.last_mut().expect("Never empty")
    }

    fn advance(&mut self) {
        *self.top_mut().offset_mut() += 1;
    }

    fn pop(&mut self) -> Option<Level<'a, C, A>> {
        // We never pop the root
        if self.levels.len() > 1 {
            self.levels.pop()
        } else {
            None
        }
    }

    fn walk<W>(&mut self, walker: &mut W) -> Option<()>
    where
        C: Compound<A> + WellFormed,
        C::Archived: ArchivedCompound<C, A> + WellArchived<C>,
        C::Leaf: WellFormed,
        A: Annotation<C::Leaf>,
        W: Walker<C, A>,
    {
        enum State<'a, C, A>
        where
            C: Compound<A> + WellFormed,
            C::Archived: WellArchived<C>,
            A: Annotation<C::Leaf>,
        {
            Init,
            Push(Level<'a, C, A>),
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
                                    let level = Level::<C, A>::new(
                                        MaybeArchived::Memory(c),
                                    );
                                    let extended: Level<'a, C, A> =
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
                                        store.get(link.ident())
                                    } else {
                                        unreachable!()
                                    }
                                }
                                _ => panic!("Invalid child found"),
                            }
                        }
                    };

                    // continued archived branch

                    let level: Level<C, A> =
                        Level::new(MaybeArchived::Archived(archived));
                    let extended: Level<'a, C, A> =
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

impl<'a, C, A> Branch<'a, C, A>
where
    C: Compound<A> + WellFormed,
    C::Archived: ArchivedCompound<C, A> + WellArchived<C>,
    C::Leaf: WellFormed,
    <C::Leaf as Archive>::Archived: WellArchived<C::Leaf>,
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
        closure: fn(MaybeArchived<'a, C::Leaf>) -> M,
    ) -> MappedBranch<'a, C, A, M>
    where
        M: 'a,
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
        C: Compound<A>,
        C::Archived: ArchivedCompound<C, A>
            + for<'any> CheckBytes<DefaultValidator<'any>>,
        C::Leaf: 'a + Archive,
        A: Annotation<C::Leaf>,
        W: Walker<C, A>,
    {
        let mut partial = PartialBranch::new(root);
        partial.walk(&mut walker).map(|()| Branch(partial))
    }

    /// Performs a tree walk, returning either a valid branch or None if the
    /// walk failed.
    pub fn walk_with_store<W>(
        root: MaybeArchived<'a, C>,
        mut walker: W,
        store: StoreRef,
    ) -> Option<Self>
    where
        C: Compound<A>,
        C::Archived: ArchivedCompound<C, A>
            + for<'any> CheckBytes<DefaultValidator<'any>>,
        C::Leaf: 'a + Archive,
        A: Annotation<C::Leaf>,
        W: Walker<C, A>,
    {
        let mut partial = PartialBranch::new_with_store(root, store);
        partial.walk(&mut walker).map(|()| Branch(partial))
    }
}

/// Reprents an immutable branch view into a collection.
///
/// Branche are always guaranteed to point at a leaf, and can be dereferenced
/// to the pointed-at leaf.
pub struct Branch<'a, C, A>(PartialBranch<'a, C, A>)
where
    C: Archive;

/// A branch that applies a map to its leaf
pub struct MappedBranch<'a, C, A, M>
where
    C: Compound<A> + Archive,
    C::Leaf: Archive,
    M: 'a,
{
    inner: Branch<'a, C, A>,
    closure: MapClosure<'a, C::Leaf, M>,
}

type MapClosure<'a, L, M> = fn(MaybeArchived<'a, L>) -> M;

impl<'a, C, A, M> MappedBranch<'a, C, A, M>
where
    C: Compound<A> + WellFormed,
    C::Archived: ArchivedCompound<C, A> + WellArchived<C>,
    C::Leaf: WellFormed,
    <C::Leaf as Archive>::Archived: WellArchived<C::Leaf>,
{
    /// Get the mapped leaf of the branch
    pub fn leaf(&'a self) -> M {
        (self.closure)(self.inner.leaf())
    }
}

pub enum BranchIterator<'a, C, A, W>
where
    C: Archive,
{
    Initial(Branch<'a, C, A>, W),
    Intermediate(Branch<'a, C, A>, W),
    Exhausted,
}

impl<'a, C, A, W> BranchIterator<'a, C, A, W>
where
    C: Compound<A> + WellFormed,
    C::Archived: ArchivedCompound<C, A> + WellArchived<C>,
    C::Leaf: WellFormed,
    <C::Leaf as Archive>::Archived: WellArchived<C::Leaf>,
{
    pub fn depth(&self) -> usize {
        match self {
            BranchIterator::Initial(b, _)
            | BranchIterator::Intermediate(b, _) => b.depth(),
            BranchIterator::Exhausted => 0,
        }
    }
}

// iterators
impl<'a, C, A> IntoIterator for Branch<'a, C, A>
where
    C: Compound<A> + WellFormed,
    C::Archived: ArchivedCompound<C, A> + WellArchived<C>,
    C::Leaf: 'a + WellFormed,
    <C::Leaf as Archive>::Archived: 'a + WellArchived<C::Leaf>,
    A: Annotation<C::Leaf>,
{
    type Item = MaybeArchived<'a, C::Leaf>;

    type IntoIter = BranchIterator<'a, C, A, All>;

    fn into_iter(self) -> Self::IntoIter {
        BranchIterator::Initial(self, All)
    }
}

impl<'a, C, A, W> Iterator for BranchIterator<'a, C, A, W>
where
    C: Compound<A> + WellFormed,
    C::Archived: ArchivedCompound<C, A> + WellArchived<C>,
    C::Leaf: 'a + WellFormed,
    <C::Leaf as Archive>::Archived: 'a + WellArchived<C::Leaf>,
    A: Annotation<C::Leaf>,
    W: Walker<C, A>,
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

pub enum MappedBranchIterator<'a, C, A, R, W>
where
    C: Archive + Compound<A>,
    C::Leaf: Archive,
{
    Initial(Branch<'a, C, A>, MapClosure<'a, C::Leaf, R>, W),
    Intermediate(Branch<'a, C, A>, MapClosure<'a, C::Leaf, R>, W),
    Exhausted,
}

impl<'a, C, A, M> IntoIterator for MappedBranch<'a, C, A, M>
where
    C: Compound<A> + WellFormed,
    C::Archived: ArchivedCompound<C, A> + WellArchived<C>,
    C::Leaf: 'a + WellFormed,
    <C::Leaf as Archive>::Archived: WellArchived<C::Leaf>,
    A: Annotation<C::Leaf>,
    M: 'a + Archive,
{
    type Item = M;

    type IntoIter = MappedBranchIterator<'a, C, A, M, All>;

    fn into_iter(self) -> Self::IntoIter {
        let MappedBranch { inner, closure } = self;
        MappedBranchIterator::Initial(inner, closure, All)
    }
}

impl<'a, C, A, M, W> Iterator for MappedBranchIterator<'a, C, A, M, W>
where
    C: Compound<A> + WellFormed,
    C::Archived: ArchivedCompound<C, A> + WellArchived<C>,
    C::Leaf: 'a + WellFormed,
    <C::Leaf as Archive>::Archived: WellArchived<C::Leaf>,
    A: Annotation<C::Leaf>,
    W: Walker<C, A>,
{
    type Item = M;

    fn next(&mut self) -> Option<Self::Item> {
        match core::mem::replace(self, Self::Exhausted) {
            Self::Initial(branch, closure, walker) => {
                *self = Self::Intermediate(branch, closure, walker);
            }
            Self::Intermediate(mut branch, closure, mut walker) => {
                branch.0.advance();
                // access partialbranch
                match branch.0.walk(&mut walker) {
                    None => {
                        *self = Self::Exhausted;
                        return None;
                    }
                    Some(_) => {
                        *self = Self::Intermediate(branch, closure, walker);
                    }
                }
            }
            Self::Exhausted => {
                return None;
            }
        }

        match self {
            Self::Intermediate(branch, closure, _) => {
                let leaf: MaybeArchived<'_, _> = branch.leaf();
                let leaf_extended: MaybeArchived<'a, _> =
                    unsafe { core::mem::transmute(leaf) };
                let mapped = closure(leaf_extended);
                Some(mapped)
            }
            _ => unreachable!(),
        }
    }
}

/// A trait for refering to a branch based solely on its leaf type
pub trait BranchRef<'a, T>
where
    T: Archive,
{
    /// Provides a reference to the leaf of the branch
    fn leaf(&self) -> MaybeArchived<T>;
}

impl<'a, C, A, T> BranchRef<'a, T>
    for MappedBranch<'a, C, A, MaybeArchived<'a, T>>
where
    C: Compound<A> + WellFormed,
    C::Archived: ArchivedCompound<C, A> + WellArchived<C>,
    C::Leaf: WellFormed,
    <C::Leaf as Archive>::Archived: WellArchived<C::Leaf>,
    T: WellFormed,
{
    fn leaf(&self) -> MaybeArchived<T> {
        let leaf: MaybeArchived<'_, C::Leaf> = self.inner.leaf();
        let leaf: MaybeArchived<'a, C::Leaf> =
            unsafe { core::mem::transmute(leaf) };
        (self.closure)(leaf)
    }
}

impl<'a, C, A> Deref for Branch<'a, C, A>
where
    C: Compound<A> + WellFormed,
    C::Leaf: Fundamental,
    <C as Archive>::Archived: ArchivedCompound<C, A> + WellArchived<C>,
{
    type Target = C::Leaf;

    fn deref(&self) -> &Self::Target {
        match self.leaf() {
            MaybeArchived::Memory(m) => &m,
            MaybeArchived::Archived(a) => &a,
        }
    }
}
