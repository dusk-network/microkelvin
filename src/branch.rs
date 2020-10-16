use core::ops::Deref;

use canonical::Store;

use crate::annotation::Annotation;
use crate::compound::{Compound, Traverse};

struct Level<'a, C, S> {
    ofs: usize,
    node: LevelNode<'a, C, S>,
}

enum LevelNode<'a, C, S> {
    Borrowed(&'a C),
    Owned(C),
    Placeholder(S),
}

pub struct Branch<'a, C, S>(Levels<'a, C, S>);

pub struct Levels<'a, C, S>(Vec<Level<'a, C, S>>);

impl<'a, C, S> Levels<'a, C, S> {
    fn new(first: Level<'a, C, S>) -> Self {
        Levels(vec![first])
    }

    fn last(&self) -> &Level<'a, C, S> {
        self.0.last().expect("always > 0 len")
    }
}

impl<'a, C, S> Branch<'a, C, S>
where
    S: Store,
    C: Compound,
{
    pub fn traverse<M: Annotation<C::Leaf>>(
        root: &'a C,
        method: &mut M,
    ) -> Result<Option<Branch<'a, C, S>>, S::Error> {
        match root.traverse(method) {
            Traverse::Leaf(ofs) => Ok(Some(Branch(
                Levels::new(Level {
                    ofs,
                    node: LevelNode::Borrowed(root),
                }),
            ))),
            _ => todo!(),
        }
    }
}

impl<'a, C, S> Deref for Branch<'a, C, S>
where
    C: Compound,
{
    type Target = C::Leaf;

    fn deref(&self) -> &Self::Target {
        unimplemented!()
    }
}
