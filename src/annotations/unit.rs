use crate::annotations::Annotation;
use crate::Compound;

impl<C> Annotation<C> for ()
where
    C: Compound,
{
    fn identity() -> () {
        ()
    }

    fn from_leaf(_: &C::Leaf) -> () {
        ()
    }

    fn from_node(_: &C) -> () {
        ()
    }
}
