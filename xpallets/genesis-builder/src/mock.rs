// Copyright 2019-2020 ChainX Project Authors. Licensed under GPL-3.0.

use crate::Trait;
use crate::*;
use chainx_primitives::{AccountId, AccountIndex, AssetId, Balance};
use frame_support::{impl_outer_origin, parameter_types, traits::Get, weights::Weight};
use sp_core::H256;
use sp_runtime::{
    testing::{Header, UintAuthorityId},
    traits::{BlakeTwo256, IdentityLookup},
    Perbill,
};
use std::{cell::RefCell, collections::HashSet};
use xp_mining_staking::SessionIndex;

pub const INIT_TIMESTAMP: u64 = 30_000;

pub(crate) type BlockNumber = u64;

// For testing the pallet, we construct most of a mock runtime. This means
// first constructing a configuration type (`Test`) which `impl`s each of the
// configuration traits of pallets we want to use.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Test;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
}

impl_outer_origin! {
    pub enum Origin for Test {}
}

impl frame_system::Trait for Test {
    type BaseCallFilter = ();
    type Origin = Origin;
    type Call = ();
    type Index = AccountIndex;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = ();
    type BlockHashCount = BlockHashCount;
    type MaximumBlockWeight = MaximumBlockWeight;
    type DbWeight = ();
    type BlockExecutionWeight = ();
    type ExtrinsicBaseWeight = ();
    type MaximumExtrinsicWeight = MaximumBlockWeight;
    type MaximumBlockLength = MaximumBlockLength;
    type AvailableBlockRatio = AvailableBlockRatio;
    type Version = ();
    type PalletInfo = ();
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
}

pub struct ExistentialDeposit;
impl Get<Balance> for ExistentialDeposit {
    fn get() -> Balance {
        EXISTENTIAL_DEPOSIT.with(|v| *v.borrow())
    }
}

impl pallet_balances::Trait for Test {
    type MaxLocks = ();
    type Balance = Balance;
    type Event = ();
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
}

parameter_types! {
    pub const MinimumPeriod: u64 = 5;
}

impl pallet_timestamp::Trait for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

/// Another session handler struct to test on_disabled.
pub struct OtherSessionHandler;
impl pallet_session::OneSessionHandler<AccountId> for OtherSessionHandler {
    type Key = UintAuthorityId;

    fn on_genesis_session<'a, I: 'a>(_: I)
    where
        I: Iterator<Item = (&'a AccountId, Self::Key)>,
        AccountId: 'a,
    {
    }

    fn on_new_session<'a, I: 'a>(_: bool, _validators: I, _: I)
    where
        I: Iterator<Item = (&'a AccountId, Self::Key)>,
        AccountId: 'a,
    {
    }

    fn on_disabled(_validator_index: usize) {}
}

impl sp_runtime::BoundToRuntimeAppPublic for OtherSessionHandler {
    type Public = UintAuthorityId;
}

pub struct Period;
impl Get<BlockNumber> for Period {
    fn get() -> BlockNumber {
        PERIOD.with(|v| *v.borrow())
    }
}

parameter_types! {
    pub const Offset: BlockNumber = 0;
    pub const UncleGenerations: u64 = 0;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(25);
}

sp_runtime::impl_opaque_keys! {
    pub struct SessionKeys {
        pub other: OtherSessionHandler,
    }
}

impl pallet_session::Trait for Test {
    type SessionManager = ();
    type Keys = SessionKeys;
    type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
    type SessionHandler = (OtherSessionHandler,);
    type Event = ();
    type ValidatorId = AccountId;
    type ValidatorIdOf = ();
    type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
    type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
    type WeightInfo = ();
}

parameter_types! {
    pub const SessionDuration: BlockNumber = 50;
    pub const MigrationSessionOffset: u32 = 500;
    pub const MinimumReferralId: u32 = 2;
    pub const MaximumReferralId: u32 = 12;
}

impl xpallet_mining_staking::Trait for Test {
    type Currency = Balances;
    type Event = ();
    type AssetMining = ();
    type MigrationSessionOffset = MigrationSessionOffset;
    type SessionDuration = SessionDuration;
    type MinimumReferralId = MinimumReferralId;
    type MaximumReferralId = MaximumReferralId;
    type SessionInterface = Self;
    type TreasuryAccount = ();
    type DetermineRewardPotAccount = ();
    type WeightInfo = ();
}

parameter_types! {
    pub const ChainXAssetId: AssetId = 0;
}

impl xpallet_assets_registrar::Trait for Test {
    type Event = ();
    type NativeAssetId = ChainXAssetId;
    type RegistrarHandler = ();
    type WeightInfo = ();
}

impl xpallet_assets::Trait for Test {
    type Event = ();
    type Currency = Balances;
    type Amount = i128;
    type TreasuryAccount = ();
    type OnCreatedAccount = frame_system::CallOnCreatedAccount<Test>;
    type OnAssetChanged = ();
    type WeightInfo = ();
}

impl xpallet_mining_asset::Trait for Test {
    type StakingInterface = Self;
    type GatewayInterface = ();
    type Event = ();
    type TreasuryAccount = ();
    type DetermineRewardPotAccount = ();
    type WeightInfo = ();
}

impl Trait for Test {}

thread_local! {
    static SESSION: RefCell<(Vec<AccountId>, HashSet<AccountId>)> = RefCell::new(Default::default());
    static SESSION_PER_ERA: RefCell<SessionIndex> = RefCell::new(3);
    static EXISTENTIAL_DEPOSIT: RefCell<Balance> = RefCell::new(0);
    static ELECTION_LOOKAHEAD: RefCell<BlockNumber> = RefCell::new(0);
    static PERIOD: RefCell<BlockNumber> = RefCell::new(1);
    static MAX_ITERATIONS: RefCell<u32> = RefCell::new(0);
}

#[derive(Default)]
pub struct ExtBuilder;

pub(crate) fn get_accounts() -> Vec<AccountId> {
    let params: xp_genesis_builder::AllParams<AccountId, Balance, Balance, Balance> =
        serde_json::from_str(include_str!(
            "../../../cli/src/res/genesis_builder_params.json"
        ))
        .expect("JSON was not well-formatted");

    params
        .balances
        .free_balances
        .iter()
        .map(|balance_info| balance_info.who.clone())
        .collect::<Vec<_>>()
}

pub(crate) fn get_accounts_info() -> std::collections::BTreeMap<AccountId, Balance> {
    let params: xp_genesis_builder::AllParams<AccountId, Balance, Balance, Balance> =
        serde_json::from_str(include_str!(
            "../../../cli/src/res/genesis_builder_params.json"
        ))
        .expect("JSON was not well-formatted");

    params
        .balances
        .free_balances
        .iter()
        .map(|balance_info| (balance_info.who.clone(), balance_info.free))
        .collect()
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let _ = env_logger::try_init();
        let mut storage = frame_system::GenesisConfig::default()
            .build_storage::<Test>()
            .unwrap();

        let params: xp_genesis_builder::AllParams<AccountId, Balance, Balance, Balance> =
            serde_json::from_str(include_str!(
                "../../../cli/src/res/genesis_builder_params.json"
            ))
            .expect("JSON was not well-formatted");

        let _ = GenesisConfig::<Test> { params }.assimilate_storage(&mut storage);

        let mut ext = sp_io::TestExternalities::from(storage);
        ext.execute_with(|| {
            let validators = Session::validators();
            SESSION.with(|x| *x.borrow_mut() = (validators.clone(), HashSet::new()));
        });

        // We consider all test to start after timestamp is initialized
        // This must be ensured by having `timestamp::on_initialize` called before
        // `staking::on_initialize`
        ext.execute_with(|| {
            System::set_block_number(1);
            Timestamp::set_timestamp(INIT_TIMESTAMP);
        });

        ext
    }
    pub fn build_and_execute(self, test: impl FnOnce() -> ()) {
        let mut ext = self.build();
        ext.execute_with(test);
    }
}

pub type System = frame_system::Module<Test>;
pub type Balances = pallet_balances::Module<Test>;
pub type Session = pallet_session::Module<Test>;
pub type Timestamp = pallet_timestamp::Module<Test>;
// pub type XAssets = xpallet_assets::Module<Test>;
// pub type XStaking = xpallet_mining_staking::Module<Test>;
