// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use arbitrary::Arbitrary;
use canonical::{Canon, Id};
use canonical_derive::Canon;
use canonical_fuzz::fuzz_canon_iterations;

#[derive(Clone, Canon, PartialEq, Debug, Arbitrary)]
struct A {
    a: u64,
    b: u64,
}

#[derive(Clone, Canon, PartialEq, Debug, Arbitrary)]
struct A2 {
    a: (),
    b: u8,
}

#[derive(Clone, Canon, PartialEq, Debug, Arbitrary)]
struct B(u64, u64);

#[derive(Clone, Canon, PartialEq, Debug, Arbitrary)]
struct C(u64);

#[derive(Clone, Canon, PartialEq, Debug, Arbitrary)]
struct D;

#[derive(Clone, Canon, PartialEq, Debug, Arbitrary)]
enum E {
    A,
    B,
}

#[derive(Clone, Canon, PartialEq, Debug)]
enum F {
    A(u64, [u64; 5]),
    B(u8),
    C(Result<u32, u32>),
}

#[derive(Clone, Canon, PartialEq, Debug, Arbitrary)]
enum G {
    A { alice: u64, bob: u8 },
    B(Option<u32>),
    C,
}

#[derive(Clone, Canon, PartialEq, Debug, Arbitrary)]
struct H<T>(T);

#[derive(Clone, Canon, PartialEq, Debug, Arbitrary)]
struct I<T>(Vec<T>);

#[derive(Clone, Canon, PartialEq, Debug, Arbitrary)]
struct J(String);

#[derive(Clone, Canon, PartialEq, Debug, Arbitrary)]
struct MonsterStruct<T> {
    a: A,
    b: B,
    c: C,
    d: D,
    e: E,
    g: G,
    h: H<T>,
    i: I<T>,
    j: J,
}

fn serialize_deserialize<T: Canon + Clone + std::fmt::Debug + PartialEq>(t: T) {
    let id = Id::new(&t);
    let restored = id.reify().unwrap();
    assert_eq!(t, restored);
}

#[test]
fn derives() {
    serialize_deserialize(A { a: 37, b: 77 });
    serialize_deserialize(A2 { a: (), b: 77 });
    serialize_deserialize(B(37, 22));
    serialize_deserialize(C(22));
    serialize_deserialize(D);
    serialize_deserialize(E::A);
    serialize_deserialize(E::B);
    serialize_deserialize(F::A(73, [0, 1, 2, 3, 4]));
    serialize_deserialize(F::B(22));
    serialize_deserialize(F::C(Ok(3213)));
    serialize_deserialize(F::C(Err(3213)));
    serialize_deserialize(G::A { alice: 73, bob: 3 });
    serialize_deserialize(G::B(Some(73)));
    serialize_deserialize(G::B(None));
    serialize_deserialize(H(73u8));
    serialize_deserialize(H(73u64));
    serialize_deserialize(H(E::B));
    serialize_deserialize(H(F::B(83)));

    serialize_deserialize(MonsterStruct {
        a: A { a: 37, b: 77 },
        b: B(37, 22),
        c: C(22),
        d: D,
        e: E::A,
        g: G::A { alice: 73, bob: 3 },
        h: H(E::B),
        i: I(vec![E::B, E::B, E::A]),
        j: J("Happy happy joy joy!".into()),
    });
}

#[test]
fn fuzzing() {
    fuzz_canon_iterations::<MonsterStruct<Option<u32>>>(32);
}
