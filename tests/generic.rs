use microkelvin::{GenericChild, GenericTree};

use canonical_fuzz::fuzz_canon;

#[test]
fn fuzz_generic_tree() {
    fuzz_canon::<GenericTree>()
}

#[test]
fn fuzz_generic_child() {
    fuzz_canon::<GenericChild>()
}
