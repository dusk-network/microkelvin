use microkelvin::collections::BTreeMap;
use microkelvin::{MaxKey, TreeViz};

use rkyv::rend::LittleEndian;

const S: i32 = 4;
const N: i32 = 16;

#[test]
fn btree_add_remove_simple() {
    let mut map =
        BTreeMap::<LittleEndian<i32>, i32, MaxKey<LittleEndian<i32>>>::new();

    for o in S..N {
        println!("\n------------\nTESTING N = {}", o);

        for i in 0..o {
            println!("insert {:?}", i);
            assert_eq!(map.insert(LittleEndian::from(i), i), None);

            map.print_tree();

            assert!(map.all_leaves_at_same_level());
        }

        for i in 0..o {
            println!("removing {:?}", i);

            assert_eq!(map.remove(&LittleEndian::from(i)), Some(i));

            map.print_tree();

            assert!(map.all_leaves_at_same_level());
        }

        assert!(map.correct_empty_state());
    }
}

#[test]
fn btree_add_remove_reverse() {
    let mut map =
        BTreeMap::<LittleEndian<i32>, i32, MaxKey<LittleEndian<i32>>>::new();

    for o in S..N {
        for i in 0..o {
            let i = o - i - 1;
            assert_eq!(map.insert(LittleEndian::from(i), i), None);

            assert!(map.all_leaves_at_same_level());
        }

        for i in 0..o {
            let i = o - i - 1;
            assert_eq!(map.remove(&LittleEndian::from(i)), Some(i));
            println!("removed {}", i);
            map.print_tree();

            assert!(map.all_leaves_at_same_level());
        }
    }

    assert!(map.correct_empty_state());
}

#[test]
fn btree_add_change_remove() {
    let mut map =
        BTreeMap::<LittleEndian<i32>, i32, MaxKey<LittleEndian<i32>>>::new();

    for o in S..N {
        println!("\n------------\nTESTING N = {}", o);

        for i in 0..o {
            println!("insert {:?}", i);
            assert_eq!(map.insert(LittleEndian::from(i), i), None);

            map.print_tree();

            assert!(map.all_leaves_at_same_level());
        }

        for i in 0..o {
            println!("re-insert {:?}", i);
            assert_eq!(map.insert(LittleEndian::from(i), i + 1), Some(i));

            map.print_tree();
        }

        for i in 0..o {
            assert_eq!(map.get(&LittleEndian::from(i)), Some(&(i + 1)));
        }

        for i in 0..o {
            println!("removing {:?}", i);

            assert_eq!(map.remove(&LittleEndian::from(i)), Some(i + 1));

            map.print_tree();

            assert!(map.all_leaves_at_same_level());
        }

        assert!(map.correct_empty_state());
    }
}
