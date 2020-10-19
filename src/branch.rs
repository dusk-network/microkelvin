use core::marker::PhantomData;
use core::ops::Deref;

use canonical::Store;

use crate::annotation::Annotated;
use crate::compound::{Child, Compound};

type Offset = usize;

pub enum Level<'a, C, S> {
    #[allow(unused)]
    Borrowed(&'a C),
    #[allow(unused)]
    Owned(C, PhantomData<S>),
}

pub struct PartialBranch<'a, C, S>(Levels<'a, C, S>);

pub struct Levels<'a, C, S>(Vec<(Offset, Level<'a, C, S>)>);

impl<'a, C, S> Levels<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    #[allow(unused)]
    pub fn new(first: Level<'a, C, S>) -> Self {
        Levels(vec![(0, first)])
    }

    #[allow(unused)]
    pub fn top(&self) -> &(Offset, Level<'a, C, S>) {
        self.0.last().expect("always > 0 len")
    }

    #[allow(unused)]
    pub fn top_mut(&mut self) -> &mut (Offset, Level<'a, C, S>) {
        self.0.last_mut().expect("always > 0 len")
    }

    #[allow(unused)]
    pub fn pop(&mut self) -> bool {
        if self.0.len() > 1 {
            self.0.pop();
            true
        } else {
            false
        }
    }

    pub fn leaf(&self) -> Option<&C::Leaf> {
        let (ofs, level) = self.top();
        match level {
            Level::Borrowed(c) => match c.child(*ofs) {
                Child::Leaf(l) => Some(l),
                _ => None,
            },
            _ => None,
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

    fn leaf(&self) -> Option<&C::Leaf> {
        self.0.leaf()
    }

    fn walk<W: FnMut(Walk<C, S>) -> Step<C, S>>(
        &mut self,
        mut walker: W,
    ) -> Result<Option<()>, S::Error> {
        loop {
            match match self.0.top_mut() {
                (ofs, Level::Borrowed(c)) => match c.child(*ofs) {
                    Child::Leaf(l) => walker(Walk::Leaf(l)),
                    Child::Node(c) => walker(Walk::Node(c)),
                    Child::EndOfNode => {
                        if !self.0.pop() {
                            return Ok(None);
                        } else {
                            Step::Next
                        }
                    }
                },
                _ => todo!(),
            } {
                Step::Found(_) => return Ok(Some(())),
                Step::Next => {
                    let (ref mut ofs, _) = self.0.top_mut();
                    *ofs += 1
                }
                _ => panic!(),
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

pub struct Branch<'a, C, S>(PartialBranch<'a, C, S>);

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
