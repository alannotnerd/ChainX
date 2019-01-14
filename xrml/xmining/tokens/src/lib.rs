// Copyright 2018 Chainpool.
//! Virtual mining for holding tokens.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate serde;

#[cfg(feature = "std")]
extern crate serde_derive;

#[macro_use]
extern crate parity_codec_derive;
extern crate parity_codec as codec;

extern crate sr_io as runtime_io;
extern crate sr_primitives as runtime_primitives;
extern crate sr_std as rstd;

#[macro_use]
extern crate srml_support as runtime_support;
extern crate srml_balances as balances;
#[cfg(test)]
extern crate srml_consensus as consensus;
extern crate srml_session as session;
extern crate srml_system as system;
extern crate srml_timestamp as timestamp;

extern crate xrml_bridge_bitcoin as bitcoin;
extern crate xrml_mining_staking as xstaking;
extern crate xrml_xaccounts as xaccounts;
extern crate xrml_xassets_assets as xassets;
extern crate xrml_xsupport as xsupport;
extern crate xrml_xsystem as xsystem;

#[cfg(test)]
extern crate substrate_primitives;

use rstd::prelude::*;
use rstd::result::Result as StdResult;
use runtime_primitives::traits::{As, CheckedSub, Zero};
use runtime_support::dispatch::Result;
use runtime_support::{StorageMap, StorageValue};

use xassets::{AssetErr, AssetType, ChainT, Token};
use xassets::{OnAssetChanged, OnAssetRegistration};
use xstaking::{Jackpot, OnReward, OnRewardCalculation, RewardHolder, VoteWeight};

/// This module only tracks the vote weight related changes.
/// All the amount related has been taken care by assets module.
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub struct PseduIntentionVoteWeight<Balance: Default, BlockNumber: Default> {
    pub jackpot: Balance,
    pub last_total_deposit_weight: u64,
    pub last_total_deposit_weight_update: BlockNumber,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Debug))]
pub struct DepositVoteWeight<BlockNumber: Default> {
    pub last_deposit_weight: u64,
    pub last_deposit_weight_update: BlockNumber,
}

/// `PseduIntentionProfs` and `DepositRecord` is to wrap the vote weight of token,
/// sharing the vote weight calculation logic originated from staking module.
pub struct PseduIntentionProfs<'a, T: Trait> {
    pub token: &'a Token,
    pub staking: &'a mut PseduIntentionVoteWeight<T::Balance, T::BlockNumber>,
}

pub struct DepositRecord<'a, T: Trait> {
    pub depositor: &'a T::AccountId,
    pub token: &'a Token,
    pub staking: &'a mut DepositVoteWeight<T::BlockNumber>,
}

impl<'a, T: Trait> VoteWeight<T::BlockNumber> for PseduIntentionProfs<'a, T> {
    fn amount(&self) -> u64 {
        xassets::Module::<T>::all_type_balance(&self.token).as_()
    }

    fn last_acum_weight(&self) -> u64 {
        self.staking.last_total_deposit_weight
    }

    fn last_acum_weight_update(&self) -> u64 {
        self.staking.last_total_deposit_weight_update.as_()
    }

    fn set_amount(&mut self, _: u64, _: bool) {}

    fn set_last_acum_weight(&mut self, latest_deposit_weight: u64) {
        self.staking.last_total_deposit_weight = latest_deposit_weight;
    }

    fn set_last_acum_weight_update(&mut self, current_block: T::BlockNumber) {
        self.staking.last_total_deposit_weight_update = current_block;
    }
}

impl<'a, T: Trait> Jackpot<T::Balance> for PseduIntentionProfs<'a, T> {
    fn jackpot(&self) -> T::Balance {
        self.staking.jackpot
    }

    fn set_jackpot(&mut self, value: T::Balance) {
        self.staking.jackpot = value;
    }
}

impl<'a, T: Trait> VoteWeight<T::BlockNumber> for DepositRecord<'a, T> {
    fn amount(&self) -> u64 {
        xassets::Module::<T>::all_type_balance_of(&self.depositor, &self.token).as_()
    }

    fn last_acum_weight(&self) -> u64 {
        self.staking.last_deposit_weight
    }

    fn last_acum_weight_update(&self) -> u64 {
        self.staking.last_deposit_weight_update.as_()
    }

    fn set_amount(&mut self, _: u64, _: bool) {}

    fn set_last_acum_weight(&mut self, latest_deposit_weight: u64) {
        self.staking.last_deposit_weight = latest_deposit_weight;
    }

    fn set_last_acum_weight_update(&mut self, current_block: T::BlockNumber) {
        self.staking.last_deposit_weight_update = current_block;
    }
}

pub trait Trait:
    xassets::Trait + xaccounts::Trait + xsystem::Trait + xstaking::Trait + bitcoin::Trait
{
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn claim(origin, token: Token) {
            let who = system::ensure_signed(origin)?;

            if <xassets::Module<T> as ChainT>::TOKEN.to_vec() == token {
                return Err("Cannot claim from native asset via tokens module.");
            }

            ensure!(
                Self::psedu_intentions().into_iter().find(|i| i.clone() == token).is_some(),
                "Cannot claim from unsupport token."
            );

            Self::apply_claim(&who, &token)?;
        }
    }
}

decl_storage! {
    trait Store for Module<T: Trait> as XTokens {
        pub PseduIntentions get(psedu_intentions): Vec<Token>;

        pub PseduIntentionProfiles get(psedu_intention_profiles): map Token => PseduIntentionVoteWeight<T::Balance, T::BlockNumber>;

        pub DepositRecords get(deposit_records): map (T::AccountId, Token) => DepositVoteWeight<T::BlockNumber>;
    }
}

impl<T: Trait> OnAssetChanged<T::AccountId, T::Balance> for Module<T> {
    fn on_move(
        token: &Token,
        from: &T::AccountId,
        _: AssetType,
        to: &T::AccountId,
        _: AssetType,
        value: T::Balance,
    ) -> StdResult<(), AssetErr> {
        // Exclude PCX and asset type changes on same account.
        if <xassets::Module<T> as ChainT>::TOKEN.to_vec() == token.clone()
            || from.clone() == to.clone()
        {
            return Ok(());
        }

        Self::update_vote_weight(from, token, value, false);
        Self::update_vote_weight(to, token, value, true);
        Ok(())
    }

    fn on_issue(target: &Token, source: &T::AccountId, value: T::Balance) -> Result {
        // Initialize vote weight of depositor
        let mut vote_weight = DepositVoteWeight::default();
        vote_weight.last_deposit_weight_update = <system::Module<T>>::block_number();
        <DepositRecords<T>>::insert((source.clone(), target.clone()), vote_weight);

        Self::issue_reward(source, target, value)
    }

    fn on_destroy(target: &Token, source: &T::AccountId, value: T::Balance) -> Result {
        Self::update_vote_weight(source, target, value, false);
        Ok(())
    }
}

impl<T: Trait> Module<T> {
    fn apply_claim(who: &T::AccountId, token: &Token) -> Result {
        let key = (who.clone(), token.clone());
        let mut p_vote_weight: PseduIntentionVoteWeight<T::Balance, T::BlockNumber> =
            <PseduIntentionProfiles<T>>::get(token);
        let mut d_vote_weight: DepositVoteWeight<T::BlockNumber> = Self::deposit_records(&key);

        {
            let mut prof = PseduIntentionProfs::<T> {
                token: token,
                staking: &mut p_vote_weight,
            };
            let mut record = DepositRecord::<T> {
                depositor: who,
                token: token,
                staking: &mut d_vote_weight,
            };

            <xstaking::Module<T>>::generic_claim(&mut record, &mut prof, who)?;
        }

        <PseduIntentionProfiles<T>>::insert(token, p_vote_weight);
        <DepositRecords<T>>::insert(key, d_vote_weight);

        Ok(())
    }

    /// Transform outside irreversible blocks to native blocks.
    fn wait_blocks(token: &Token) -> StdResult<u64, &'static str> {
        let seconds_per_block: T::Moment = timestamp::Module::<T>::block_period();
        match token.as_slice() {
            // btc
            <bitcoin::Module<T> as ChainT>::TOKEN => {
                let irr_block: u32 = <bitcoin::Module<T>>::irr_block();
                let seconds = (irr_block * 10 * 60) as u64;
                Ok(seconds / seconds_per_block.as_())
            }
            _ => Err("This token is not supported."),
        }
    }

    fn issue_reward(source: &T::AccountId, token: &Token, value: T::Balance) -> Result {
        let psedu_intention = Self::psedu_intention_profiles(token);
        if psedu_intention.last_total_deposit_weight == 0 {
            return Err("token's last_total_deposit_weight is zero.");
        }
        let blocks = Self::wait_blocks(token)?;
        let reward = T::Balance::sa(
            psedu_intention.jackpot.as_() * blocks * value.as_()
                / psedu_intention.last_total_deposit_weight,
        );

        Self::cut_jackpot(token, &reward);
        <xassets::Module<T>>::pcx_issue(source, reward)?;

        Ok(())
    }

    /// Reward to depositor from the jackpot directly,
    /// There is no vote weight related changes with regard to psedu intention itself and other depositors.
    fn cut_jackpot(token: &Token, value: &T::Balance) {
        let mut psedu_intention = Self::psedu_intention_profiles(token);
        psedu_intention.jackpot = match psedu_intention.jackpot.checked_sub(value) {
            Some(n) => n,
            None => Zero::zero(),
        };

        <PseduIntentionProfiles<T>>::insert(token, psedu_intention);
    }

    /// Actually update the vote weight and nomination balance of source and target.
    fn update_vote_weight(source: &T::AccountId, target: &Token, value: T::Balance, to_add: bool) {
        let key = (source.clone(), target.clone());
        let mut p_vote_weight: PseduIntentionVoteWeight<T::Balance, T::BlockNumber> =
            <PseduIntentionProfiles<T>>::get(target);
        let mut d_vote_weight: DepositVoteWeight<T::BlockNumber> = Self::deposit_records(&key);

        {
            let mut prof = PseduIntentionProfs::<T> {
                token: target,
                staking: &mut p_vote_weight,
            };
            let mut record = DepositRecord::<T> {
                depositor: source,
                token: target,
                staking: &mut d_vote_weight,
            };

            <xstaking::Module<T>>::update_vote_weight_both_way(
                &mut prof,
                &mut record,
                value.as_(),
                to_add,
            );
        }

        <PseduIntentionProfiles<T>>::insert(target, p_vote_weight);
        <DepositRecords<T>>::insert(key, d_vote_weight);
    }
}

impl<T: Trait> OnAssetRegistration for Module<T> {
    fn register_psedu_intention(token: Token) -> Result {
        ensure!(
            Self::psedu_intentions()
                .into_iter()
                .find(|i| *i == token)
                .is_none(),
            "Cannot register psedu intention repeatedly."
        );
        <PseduIntentions<T>>::mutate(|i| i.push(token));
        Ok(())
    }
}

impl<T: Trait> OnRewardCalculation<T::AccountId, T::Balance> for Module<T> {
    fn psedu_intentions_info() -> Vec<(RewardHolder<T::AccountId>, T::Balance)> {
        Self::psedu_intentions()
            .into_iter()
            .filter(|token| <xassets::Module<T>>::pcx_price_for(token).is_some())
            .map(|token| {
                let price = <xassets::Module<T>>::pcx_price_for(&token).unwrap_or(Zero::zero());
                let amount = <xassets::Module<T>>::all_type_balance(&token);

                // Apply discount for psedu intentions
                // TODO need to be configurable?
                let stake = T::Balance::sa(price.as_() * amount.as_() * 3 / 10);

                (RewardHolder::PseduIntention(token), stake)
            })
            .collect()
    }
}

impl<T: Trait> OnReward<T::AccountId, T::Balance> for Module<T> {
    fn reward(token: &Token, value: T::Balance) {
        PseduIntentionProfiles::<T>::mutate(token, |pprof| {
            pprof.jackpot += value;
        });
    }
}