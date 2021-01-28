use super::{Ann, Annotation};

impl<L> Annotation<L> for () {
    fn from_leaf(_: &L) -> Self {
        ()
    }

    fn combine(_: &[Ann<Self>]) -> Self {
        ()
    }
}
