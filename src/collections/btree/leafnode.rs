use core::borrow::Borrow;
use core::cmp::Ordering;
use core::fmt::Debug;
use core::mem;

use rkyv::{Archive, Deserialize, Serialize};

use bytecheck::CheckBytes;

use crate::Fundamental;

use super::btreemap::{Insert, Pair, Remove};

fn leaf_search<'a, O, K, V>(o: &'a O) -> impl Fn(&Pair<K, V>) -> Ordering + 'a
where
    K: 'a + Borrow<O>,
    O: Ord,
{
    move |p: &Pair<K, V>| p.k.borrow().cmp(o)
}

#[derive(Archive, Clone, Serialize, Deserialize, Debug)]
#[archive_attr(derive(CheckBytes))]
pub struct LeafNode<K, V, const LE: usize>(Vec<Pair<K, V>>);

impl<K, V, const N: usize> Default for LeafNode<K, V, N> {
    fn default() -> Self {
        LeafNode(vec![])
    }
}

impl<K, V, const LE: usize> LeafNode<K, V, LE>
where
    K: Fundamental + Debug,
    V: Debug,
{
    #[inline(always)]
    fn split_point() -> usize {
        (LE + 1) / 2
    }

    #[inline(always)]
    fn full(&self) -> bool {
        self.len() == LE
    }

    #[inline(always)]
    fn underflow(&self) -> bool {
        self.len() <= LE / 2
    }

    #[inline(always)]
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    #[inline(always)]
    fn remaining_capacity(&self) -> usize {
        LE - self.len()
    }

    fn split_off(&mut self, at: usize) -> Self {
        LeafNode(self.0.split_off(at))
    }

    pub(crate) fn insert_leaf(&mut self, k: K, v: V) -> Insert<V, Self>
    where
        K: Ord,
    {
        println!("insert leaf");
        match self.0.binary_search_by(leaf_search(&k)) {
            Ok(idx) => Insert::Replaced(mem::replace(&mut self.0[idx].v, v)),
            Err(idx) => {
                if self.full() {
                    println!("orgo");

                    let mut point = Self::split_point();

                    if idx < point {
                        point -= 1
                    }

                    let mut rest = self.0.split_off(point);

                    match rest[0].k.cmp(&k) {
                        Ordering::Greater => {
                            self.0.insert(idx, Pair { k, v });
                        }
                        Ordering::Equal => {
                            unreachable!("Equal keys can never grow the nodes")
                        }
                        Ordering::Less => {
                            rest.insert(idx - point, Pair { k, v })
                        }
                    }
                    Insert::Split(LeafNode(rest))
                } else {
                    self.0.insert(idx, Pair { k, v });
                    Insert::Ok
                }
            }
        }
    }

    pub(crate) fn append(&mut self, mut other: Self) -> Option<Self> {
        let cap = self.remaining_capacity();
        let needed = other.len();

        println!("leafnode: append\n{:?}\nto self \n{:?}", other, self);

        if cap >= needed {
            self.0.append(&mut other.0);
            None
        } else {
            //other.0.prepend(self);

            println!("{} {}\n{:?}\n{:?}", cap, needed, self, other);

            todo!();

            // // make room by splitting.
            // println!("\n\n--torka");

            // let total_len = self.len() + other.len();
            // let ideal_len = total_len / 2;

            // let self_len = self.len();
            // let other_len = other.len();

            // dbg!(total_len, ideal_len, self_len, other_len);

            // if self.len() >= ideal_len {
            //     println!("skorgo");

            //     let split_at = ideal_len - other.len();
            //     let last = self.split_off(split_at);

            //     debug_assert!(self.append(other).is_none());

            //     println!("self {:?}\nlast {:?}", self, last);

            //     Some(last)
            // } else {
            //     println!("gorgo");

            //     let split_at = other.len() - ideal_len;

            //     let mut last = other.split_off(split_at);

            //     dbg!(split_at, self, last);

            //     todo!()

            //     //Some(last)
            // }
        }
    }

    pub(crate) fn prepend(&mut self, other: &mut Self) -> Result<(), ()> {
        let cap = self.remaining_capacity();
        let needed = other.len();

        if cap >= needed {
            other.0.append(&mut self.0);
            mem::swap(other, self);
            Ok(())
        } else {
            // make room by splitting.

            println!("gorka");

            let total_len = self.len() + other.len();

            let ideal_len = total_len / 2;

            let split_at = ideal_len - other.len();

            let last = self.split_off(split_at);

            debug_assert!(self.prepend(other).is_none());

            println!("returning {:?}", last);

            Some(last)
        }
    }

    pub(crate) fn get<O>(&self, o: &O) -> Option<&V>
    where
        O: Ord,
        K: Ord + Borrow<O>,
    {
        if let Ok(idx) = self.0.binary_search_by(leaf_search(o)) {
            Some(&self.0[idx].v)
        } else {
            None
        }
    }

    pub(crate) fn get_leaf(&self, ofs: usize) -> Option<&Pair<K, V>> {
        self.0.get(ofs)
    }

    pub(crate) fn remove<O>(&mut self, o: &O) -> Remove<V>
    where
        K: Borrow<O>,
        O: Ord + Debug,
    {
        println!("remove {:?} from {:?}", o, self);

        if let Ok(idx) = self.0.binary_search_by(leaf_search(o)) {
            let removed = self.0.remove(idx).v;

            println!("after remove leaf {:?}", self);

            if self.underflow() {
                println!("underflow in leaf node");
                Remove::Underflow(removed)
            } else {
                Remove::Removed(removed)
            }
        } else {
            Remove::None
        }
    }
}
