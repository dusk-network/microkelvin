// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

mod linked_list;

#[cfg(not(feature = "persistence"))]
mod non_persist_tests {

    use super::*;
    use canonical::{Canon, CanonError, EncodeToVec, Source};
    use linked_list::LinkedList;

    #[test]
    fn link() -> Result<(), CanonError> {
        let mut list = LinkedList::<u64, ()>::new();

        let n: u64 = 64;

        for i in 0..n {
            list.push(i);
        }

        let encoded = list.encode_to_vec();

        let mut source = Source::new(&encoded);

        let mut decoded = LinkedList::<u64, ()>::decode(&mut source)?;

        for i in 0..n {
            assert_eq!(decoded.pop()?, Some(n - i - 1))
        }

        Ok(())
    }
}
