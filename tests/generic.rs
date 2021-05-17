use microkelvin::GenericTree;

use canonical_fuzz::fuzz_canon;

#[test]
fn fuzz_generic_tree() {
    fuzz_canon::<GenericTree>()
}
