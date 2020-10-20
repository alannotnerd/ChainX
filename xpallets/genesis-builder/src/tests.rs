// Copyright 2019-2020 ChainX Project Authors. Licensed under GPL-3.0.

use super::*;
use crate::mock::*;

#[test]
fn test_staking() {
    ExtBuilder::default().build_and_execute(|| {
        xpallet_mining_staking::Nominations::<Test>::iter_prefix(&who)
            .map(|(validator, ledger)| (validator, ledger))
            .collect()
    });
}
