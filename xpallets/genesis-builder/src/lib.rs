// Copyright 2020 ChainX Project Authors. Licensed under GPL-3.0.

//! This crate provides the feature of initializing the genesis state from ChainX 1.0.

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use sp_std::prelude::*;

use frame_support::{decl_module, decl_storage};

#[cfg(feature = "std")]
use xp_genesis_builder::AllParams;
use xpallet_assets::BalanceOf as AssetBalanceOf;
use xpallet_mining_staking::BalanceOf as StakingBalanceOf;
use xpallet_support::{debug, info};

pub trait Trait:
    pallet_balances::Trait + xpallet_mining_asset::Trait + xpallet_mining_staking::Trait
{
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {}
}

decl_storage! {
    trait Store for Module<T: Trait> as XGenesisBuilder {}
    add_extra_genesis {
        config(params): AllParams<T::AccountId, T::Balance, AssetBalanceOf<T>, StakingBalanceOf<T>>;
        build(|config| {
            use crate::genesis::{xassets, balances, xstaking, xmining_asset};

            let now = std::time::Instant::now();

            let accounts = &config.params
        .balances
        .free_balances
        .iter()
        .map(|balance_info| balance_info.who.clone())
        .collect::<Vec<_>>();

            balances::initialize::<T>(&config.params.balances);
            xassets::initialize::<T>(&config.params.xassets);
            xstaking::initialize::<T>(&config.params.xstaking, accounts);
            xmining_asset::initialize::<T>(&config.params.xmining_asset);

            info!(
                "Took {:?}ms to orchestrate the exported state from ChainX 1.0",
                now.elapsed().as_millis()
            );
        })
    }
}

#[cfg(feature = "std")]
mod genesis {
    pub mod balances {
        use crate::Trait;
        use frame_support::traits::StoredMap;
        use pallet_balances::AccountData;
        use xp_genesis_builder::{BalancesParams, FreeBalanceInfo, WellknownAccounts};
        use xpallet_support::traits::TreasuryAccount;

        /// Returns the validator account by the given reward pot account.
        fn validator_for<'a, T: Trait, I: Iterator<Item = &'a (T::AccountId, T::AccountId)>>(
            target_pot: &T::AccountId,
            mut pots: I,
        ) -> Option<&'a T::AccountId> {
            pots.find(|(pot, _)| *pot == *target_pot)
                .map(|(_, validator)| validator)
        }

        pub fn initialize<T: Trait>(params: &BalancesParams<T::AccountId, T::Balance>) {
            let BalancesParams {
                free_balances,
                wellknown_accounts,
            } = params;

            let WellknownAccounts {
                legacy_council,
                legacy_team,
                legacy_pots,
            } = wellknown_accounts;

            let set_free_balance = |who: &T::AccountId, free: &T::Balance| {
                T::AccountStore::insert(
                    who,
                    AccountData {
                        free: *free,
                        ..Default::default()
                    },
                )
            };

            let treasury_account =
                <T as xpallet_mining_staking::Trait>::TreasuryAccount::treasury_account();

            let vesting_account = xpallet_mining_staking::Module::<T>::vesting_account();

            let mut total = T::Balance::default();
            for FreeBalanceInfo { who, free } in free_balances {
                total += *free;
                if *who == *legacy_council {
                    set_free_balance(&treasury_account, free);
                } else if *who == *legacy_team {
                    set_free_balance(&vesting_account, free);
                } else if let Some(validator) = validator_for::<T, _>(who, legacy_pots.iter()) {
                    let new_pot = xpallet_mining_staking::Module::<T>::reward_pot_for(validator);
                    set_free_balance(&new_pot, free);
                } else {
                    set_free_balance(who, free);
                }
            }
            println!("--------------- inserted total:{:?}", total);
        }
    }

    pub mod xassets {
        use crate::{AssetBalanceOf, Trait};
        use xp_genesis_builder::FreeBalanceInfo;
        use xp_protocol::X_BTC;

        pub fn initialize<T: Trait>(
            xbtc_assets: &[FreeBalanceInfo<T::AccountId, AssetBalanceOf<T>>],
        ) {
            for FreeBalanceInfo { who, free } in xbtc_assets {
                xpallet_assets::Module::<T>::force_set_free_balance(&X_BTC, who, *free);
            }
        }
    }

    pub mod xstaking {
        use crate::{StakingBalanceOf, Trait};
        use frame_support::storage::{
            IterableStorageDoubleMap, IterableStorageMap, StorageDoubleMap,
        };
        use xp_genesis_builder::{Nomination, NominatorInfo, XStakingParams};
        use xpallet_support::{debug, error, info};

        pub fn initialize<T: Trait>(
            params: &XStakingParams<T::AccountId, StakingBalanceOf<T>>,
            accounts: &[T::AccountId],
        ) {
            let XStakingParams {
                validators,
                nominators,
            } = params;

            let genesis_validators = validators.iter().map(|v| v.who.clone()).collect::<Vec<_>>();

            // Firstly register the genesis validators.
            xpallet_mining_staking::Module::<T>::initialize_validators(validators)
                .expect("Failed to initialize genesis staking validators");

            let calc_total_weight = || {
                let mut total = 0u128;
                for account in accounts.iter() {
                    let mut contained = 0u32;
                    let mut total1 = 0u128;
                    for validator in genesis_validators.iter() {
                        if xpallet_mining_staking::Nominations::<T>::contains_key(
                            account, validator,
                        ) {
                            contained += 1;
                            let ledger =
                                xpallet_mining_staking::Nominations::<T>::get(account, validator);
                            total1 += ledger.last_vote_weight;
                        }
                    }

                    /*
                    let mut contained2 = 0u32;
                    let mut total2 = 0u128;
                    for (validator, ledger) in
                        xpallet_mining_staking::Nominations::<T>::iter_prefix(account)
                    {
                        contained2 += 1;
                        total2 += ledger.last_vote_weight;
                    }
                    if total1 != total2 {
                        panic!("total1 != total2");
                    }
                    if contained != 0 {
                        if contained != contained2 {
                            panic!("----------------- contained != contained2");
                        }
                    }
                    */

                    total += total1;
                }
                total
            };

            let mut total = 0u128;
            let mut last_total = 0u128;

            // let mut diffs = vec![];
            let mut last_diff = 0u128;
            // Then mock the validator bond themselves and set the vote weights.
            for NominatorInfo {
                nominator,
                nominations,
            } in nominators
            {
                for Nomination {
                    nominee,
                    nomination,
                    weight,
                } in nominations
                {
                    // Not all `nominee` are in `genesis_validators` because the dead
                    // validators in 1.0 have been dropped.
                    if genesis_validators.contains(nominee) {
                        /*
                        let last_calc_total = calc_total_weight();
                        info!("----------- last_calc_total:{:?}", last_calc_total);
                        if last_calc_total != total {
                            error!("last_calc_total:{:?}, total:{:?}", last_calc_total, total);
                        }

                        info!("total before: {:?}", total);
                        total += *weight;
                        info!("total  after: {:?}", total);

                        info!(
                            "setting nominator:{:?}, nominee:{:?}, weight:{:?}",
                            nominator, nominee, weight
                        );
                        */

                        xpallet_mining_staking::Module::<T>::force_set_nominator_vote_weight(
                            nominator, nominee, *weight,
                        );

                        assert!(
                            xpallet_mining_staking::Nominations::<T>::get(nominator, nominee)
                                .last_vote_weight
                                == *weight
                        );

                        /*
                        let ledger_in_storage =
                            xpallet_mining_staking::Nominations::<T>::get(nominator, nominee);
                        info!("--- inserted: {:?}", ledger_in_storage);

                        let calculated_result = calc_total_weight();
                        if calculated_result != total {
                            let new_diff = total - calculated_result;
                            if new_diff != last_diff {
                                diffs.push((nominator.clone(), nominee.clone(), ledger_in_storage));
                                last_diff = new_diff;
                            }
                            error!(
                                "calc_total_weight:{:?}, total: {:?}, diff: {:?}",
                                calculated_result,
                                total,
                                total - calculated_result
                            );
                            // panic!("total weight incorrect");
                        }
                        */

                        // Skip the validator self-bonding as it has already been processed
                        // in initialize_validators()
                        if *nominee != *nominator {
                            xpallet_mining_staking::Module::<T>::force_bond(
                                nominator,
                                nominee,
                                *nomination,
                            )
                            .expect("force validator self-bond can not fail; qed");
                        }
                    }
                }
            }
            /*
            info!("diffs:{:#?}", diffs);

            for (nominator, nominee, ledger) in diffs {
                info!(
                    "setting again nominator:{:?}, nominee:{:?}, ledger:{:?}",
                    nominator, nominee, ledger
                );

                info!(
                    "in storage: {:?}",
                    xpallet_mining_staking::Nominations::<T>::get(&nominator, &nominee)
                        .last_vote_weight
                );
                xpallet_mining_staking::Module::<T>::force_set_nominator_vote_weight(
                    &nominator,
                    &nominee,
                    ledger.last_vote_weight,
                );

                let last_calc_total = calc_total_weight();
                info!("----------- last_calc_total:{:?}", last_calc_total);
                if last_calc_total != total {
                    error!(
                        "again last_calc_total:{:?}, total:{:?}",
                        last_calc_total, total
                    );
                }
            }
            */
        }
    }

    pub mod xmining_asset {
        use crate::Trait;
        use xp_genesis_builder::{XBtcMiner, XMiningAssetParams};
        use xp_protocol::X_BTC;

        /// Mining asset module initialization only involves the mining weight.
        /// - Set xbtc mining asset weight.
        /// - Set xbtc miners' weight.
        pub fn initialize<T: Trait>(params: &XMiningAssetParams<T::AccountId>) {
            let XMiningAssetParams {
                xbtc_miners,
                xbtc_info,
            } = params;
            let current_block = frame_system::Module::<T>::block_number();
            for XBtcMiner { who, weight } in xbtc_miners {
                xpallet_mining_asset::Module::<T>::force_set_miner_mining_weight(
                    who,
                    &X_BTC,
                    *weight,
                    current_block,
                );
            }
            xpallet_mining_asset::Module::<T>::force_set_asset_mining_weight(
                &X_BTC,
                xbtc_info.weight,
                current_block,
            );
        }
    }
}
