use crate::{Child, Compound, MaybeStored, WellFormed};

use core::fmt;

pub trait TreeViz<A> {
    fn print_tree(&self) {
        println!("{}", self.to_string());
    }

    fn to_string(&self) -> String {
        let mut s = String::new();
        self.treeify(&mut s, 0);
        s
    }

    fn treeify(&self, s: &mut String, ident: usize);
}

impl<C, A> TreeViz<A> for C
where
    C: WellFormed + Compound<A>,
    C::Leaf: fmt::Debug,
    A: fmt::Debug,
{
    fn treeify(&self, s: &mut String, ident: usize) {
        *s += "\n";
        for _ in 0..ident {
            *s += "  ";
        }
        *s += "[";
        for i in 0.. {
            match self.child(i) {
                Child::Leaf(leaf) => *s += &format!("{:?}", leaf),
                Child::Link(link) => match link.inner() {
                    MaybeStored::Memory(c) => c.treeify(s, ident + 1),
                    MaybeStored::Stored(_) => todo!(),
                },

                Child::Empty => *s += "_",
                Child::End => {
                    break;
                }
            }

            if let Child::End = self.child(i + 1) {
            } else {
                *s += ", "
            }
        }

        *s += "]";
    }
}
