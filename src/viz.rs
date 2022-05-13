use crate::{Child, Compound, MaybeStored, WellFormed};

use core::fmt;

pub trait TreeViz<A> {
    fn treeify(&self, s: &mut fmt::Formatter, ident: usize) -> fmt::Result;
}

impl<C, A> TreeViz<A> for C
where
    C: WellFormed + Compound<A>,
    C::Leaf: fmt::Debug,
    A: fmt::Debug,
{
    fn treeify(&self, s: &mut fmt::Formatter, ident: usize) -> fmt::Result {
        write!(s, "\n")?;
        for _ in 0..ident {
            write!(s, "  ")?;
        }
        write!(s, "[")?;
        for i in 0.. {
            match self.child(i) {
                Child::Leaf(leaf) => write!(s, "{:?}", leaf)?,
                Child::Link(link) => match link.inner() {
                    MaybeStored::Memory(c) => c.treeify(s, ident + 1)?,
                    MaybeStored::Stored(_) => todo!(),
                },

                Child::Empty => write!(s, "_")?,
                Child::End => {
                    break;
                }
            };

            if let Child::End = self.child(i + 1) {
            } else {
                write!(s, ", ")?;
            }
        }

        write!(s, "]")
    }
}
