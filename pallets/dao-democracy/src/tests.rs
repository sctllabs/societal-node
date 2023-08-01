// This file is part of Substrate.

// Copyright (C) 2017-2022 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// Changes made comparing to the RuntimeOriginal tests of the FRAME pallet-collective:
// - added DAO pallet configuration as DAO provider
// - using DAO as a parameter for pallet functions

//! DAO Democracy pallet tests.

use super::*;
use crate as pallet_democracy;
use dao_primitives::{ContainsDaoMember, InitializeDaoMembers, RemoveDaoMembers};
use frame_support::{
	assert_noop, assert_ok, ord_parameter_types, parameter_types,
	traits::{
		AsEnsureOriginWithArg, ConstU32, ConstU64, Contains, EqualPrivilegeOnly, OnInitialize,
		SortedMembers, StorePreimage,
	},
	weights::{constants::WEIGHT_REF_TIME_PER_SECOND, Weight},
	PalletId,
};
use frame_system::{EnsureRoot, EnsureSigned, EnsureSignedBy};
use pallet_dao_assets::Error as AssetsError;
use serde_json::{json, Value};
use sp_core::{
	crypto::Ss58Codec,
	sr25519::{Public, Signature},
	ConstU128, H256,
};
use sp_runtime::{
	testing::{Header, TestXt},
	traits::{
		BadOrigin, BlakeTwo256, Extrinsic as ExtrinsicT, IdentifyAccount, IdentityLookup, Verify,
	},
	Perbill,
};
use std::collections::HashMap;

mod cancellation;
mod decoders;
mod delegation;
mod external_proposing;
mod fast_tracking;
mod lock_voting;
mod public_proposals;
mod scheduling;
mod voting;

const AYE: Vote = Vote { aye: true, conviction: Conviction::None };
const NAY: Vote = Vote { aye: false, conviction: Conviction::None };
const BIG_AYE: Vote = Vote { aye: true, conviction: Conviction::Locked1x };
const BIG_NAY: Vote = Vote { aye: false, conviction: Conviction::Locked1x };

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Preimage: pallet_preimage,
		Scheduler: pallet_scheduler::{Pallet, Call, Storage, Event<T>},
		Democracy: pallet_democracy::{Pallet, Call, Storage, Event<T>},
		Assets: pallet_dao_assets::{Pallet, Call, Storage, Config<T>, Event<T>},
		Dao: pallet_dao::{Pallet, Call, Storage, Event<T>, Config<T>},
	}
);

// Test that a filtered call can be dispatched.
pub struct BaseFilter;
impl Contains<RuntimeCall> for BaseFilter {
	fn contains(call: &RuntimeCall) -> bool {
		!matches!(call, &RuntimeCall::Balances(pallet_balances::Call::set_balance { .. }))
	}
}

const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

parameter_types! {
	pub BlockWeights: frame_system::limits::BlockWeights =
		frame_system::limits::BlockWeights::with_sensible_defaults(
			Weight::from_parts(2u64 * WEIGHT_REF_TIME_PER_SECOND, u64::MAX),
			NORMAL_DISPATCH_RATIO,
		);
}
impl frame_system::Config for Test {
	type BaseCallFilter = BaseFilter;
	type BlockWeights = BlockWeights;
	type BlockLength = ();
	type DbWeight = ();
	type RuntimeOrigin = RuntimeOrigin;
	type Index = u64;
	type BlockNumber = u64;
	type RuntimeCall = RuntimeCall;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u128>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}
parameter_types! {
	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) * BlockWeights::get().max_block;
}

impl pallet_preimage::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type Currency = Balances;
	type ManagerOrigin = EnsureRoot<AccountId>;
	type BaseDeposit = ConstU128<0>;
	type ByteDeposit = ConstU128<0>;
}

impl pallet_scheduler::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type PalletsOrigin = OriginCaller;
	type RuntimeCall = RuntimeCall;
	type MaximumWeight = MaximumSchedulerWeight;
	type ScheduleOrigin = EnsureRoot<AccountId>;
	type MaxScheduledPerBlock = ConstU32<100>;
	type WeightInfo = ();
	type OriginPrivilegeCmp = EqualPrivilegeOnly;
	type Preimages = ();
}

impl pallet_balances::Config for Test {
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type MaxLocks = ConstU32<10>;
	type Balance = u128;
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = System;
	type WeightInfo = ();
}
parameter_types! {
	pub static PreimageByteDeposit: u64 = 0;
	pub static InstantAllowed: bool = false;
}
ord_parameter_types! {
	pub const Alice: Public = Public::from_string("/Alice").ok().unwrap();
	pub const Bob: Public = Public::from_string("/Bob").ok().unwrap();
	pub const Charlie: Public = Public::from_string("/Charlie").ok().unwrap();
	pub const Dave: Public = Public::from_string("/Dave").ok().unwrap();
	pub const Eve: Public = Public::from_string("/Eve").ok().unwrap();
	pub const Ferdie: Public = Public::from_string("/Ferdie").ok().unwrap();
}
pub struct AliceToFerdie;
impl SortedMembers<Public> for AliceToFerdie {
	fn sorted_members() -> Vec<Public> {
		vec![
			Public::from_string("/Alice").ok().unwrap(),
			Public::from_string("/Bob").ok().unwrap(),
			Public::from_string("/Charlie").ok().unwrap(),
			Public::from_string("/Dave").ok().unwrap(),
			Public::from_string("/Eve").ok().unwrap(),
		]
	}

	fn contains(_t: &Public) -> bool {
		true
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn add(_m: &Public) {}
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = pallet_balances::Pallet<Self>;
	type MaxDeposits = ConstU32<1000>;
	type MaxBlacklisted = ConstU32<5>;
	type ExternalOrigin = AsEnsureOriginWithArg<EnsureRoot<AccountId>>;
	type ExternalMajorityOrigin = AsEnsureOriginWithArg<EnsureRoot<AccountId>>;
	type ExternalDefaultOrigin = AsEnsureOriginWithArg<EnsureRoot<AccountId>>;
	type FastTrackOrigin = AsEnsureOriginWithArg<EnsureRoot<AccountId>>;
	type CancellationOrigin = AsEnsureOriginWithArg<EnsureRoot<AccountId>>;
	type BlacklistOrigin = AsEnsureOriginWithArg<EnsureRoot<AccountId>>;
	type CancelProposalOrigin = AsEnsureOriginWithArg<EnsureRoot<AccountId>>;
	type VetoOrigin = EnsureSignedBy<AliceToFerdie, AccountId>;
	type Slash = ();
	type InstantOrigin = AsEnsureOriginWithArg<EnsureRoot<AccountId>>;
	type Scheduler = Scheduler;
	type MaxVotes = ConstU32<100>;
	type PalletsOrigin = OriginCaller;
	type WeightInfo = ();
	type MaxProposals = ConstU32<100>;
	type Preimages = Preimage;
	type Assets = Assets;
	type Proposal = RuntimeCall;
	type ProposalMetadataLimit = ConstU32<750>;
	type DaoProvider = Dao;
}

pub struct TestCouncilProvider;
impl InitializeDaoMembers<u32, AccountId> for TestCouncilProvider {
	fn initialize_members(
		dao_id: u32,
		source_members: Vec<AccountId>,
	) -> Result<(), DispatchError> {
		let mut members = HashMap::new();
		members.insert(dao_id, source_members.clone());

		Members::set(members);

		Ok(())
	}
}

impl ContainsDaoMember<u32, AccountId> for TestCouncilProvider {
	fn contains(_dao_id: u32, _who: &AccountId) -> Result<bool, DispatchError> {
		Ok(true)
	}
}

impl RemoveDaoMembers<u32> for TestCouncilProvider {
	fn remove_members(_dao_id: u32, _purge: bool) -> Result<(), DispatchError> {
		Ok(())
	}
}

type Extrinsic = TestXt<RuntimeCall, ()>;

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Test
where
	RuntimeCall: From<LocalCall>,
{
	fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
		call: RuntimeCall,
		_public: <Signature as Verify>::Signer,
		_account: AccountId,
		nonce: u64,
	) -> Option<(RuntimeCall, <Extrinsic as ExtrinsicT>::SignaturePayload)> {
		Some((call, (nonce, ())))
	}
}

impl frame_system::offchain::SigningTypes for Test {
	type Public = <Signature as Verify>::Signer;
	type Signature = Signature;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Test
where
	RuntimeCall: From<C>,
{
	type OverarchingCall = RuntimeCall;
	type Extrinsic = Extrinsic;
}

impl From<RawOrigin<Public>> for OriginCaller {
	fn from(_value: RawOrigin<Public>) -> Self {
		OriginCaller::system(frame_system::RawOrigin::Root)
	}
}

parameter_types! {
	pub const DaoPalletId: PalletId = PalletId(*b"py/sctld");
	pub static Members: HashMap<u32, Vec<AccountId>> = HashMap::new();
}

impl pallet_dao::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type PalletId = DaoPalletId;
	type Currency = pallet_balances::Pallet<Self>;
	type DaoNameLimit = ConstU32<20>;
	type DaoStringLimit = ConstU32<100>;
	type DaoMetadataLimit = ConstU32<750>;
	type AssetId = u128;
	type Balance = u128;
	type CouncilProvider = TestCouncilProvider;
	type AssetProvider = Assets;
	type AuthorityId = pallet_dao::crypto::TestAuthId;
	type DaoMaxCouncilMembers = ConstU32<100>;
	type DaoMaxTechnicalCommitteeMembers = ConstU32<100>;
	type DaoMaxPendingItems = ConstU32<100>;
	type TechnicalCommitteeProvider = TestCouncilProvider;
	type OffchainEthService = ();
	type RuntimeCall = RuntimeCall;
	type DaoMinTreasurySpendPeriod = ConstU32<20>;
	type ApproveOrigin = AsEnsureOriginWithArg<EnsureRoot<AccountId>>;
	type Scheduler = Scheduler;
	type PalletsOrigin = OriginCaller;
	type Preimages = ();
	type SpendDaoFunds = ();
	type DaoDemocracyProvider = Democracy;
	type DaoEthGovernanceProvider = ();
	type DaoBountiesProvider = ();
	type DaoSubscriptionProvider = ();
	type WeightInfo = ();

	#[cfg(feature = "runtime-benchmarks")]
	type DaoReferendumBenchmarkHelper = ();
}

impl pallet_dao_assets::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = u128;
	type RemoveItemsLimit = ConstU32<1000>;
	type AssetId = u128;
	type AssetIdParameter = codec::Compact<u128>;
	type Currency = Balances;
	type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
	type ForceOrigin = EnsureRoot<AccountId>;
	type AssetDeposit = ConstU128<0>;
	type AssetAccountDeposit = ConstU128<10>;
	type MetadataDepositBase = ConstU128<0>;
	type MetadataDepositPerByte = ConstU128<0>;
	type ApprovalDeposit = ConstU128<0>;
	type StringLimit = ConstU32<50>;
	type Freezer = Assets;
	type Extra = ();
	type CallbackHandle = ();
	type WeightInfo = pallet_dao_assets::weights::SubstrateWeight<Test>;
	type MaxLocks = ConstU32<10>;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
	pallet_balances::GenesisConfig::<Test> {
		balances: vec![
			(Public::from_string("/Alice").ok().unwrap(), 10),
			(Public::from_string("/Bob").ok().unwrap(), 20),
			(Public::from_string("/Charlie").ok().unwrap(), 30),
			(Public::from_string("/Dave").ok().unwrap(), 40),
			(Public::from_string("/Eve").ok().unwrap(), 50),
			(Public::from_string("/Ferdie").ok().unwrap(), 60),
		],
	}
	.assimilate_storage(&mut t)
	.unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

#[test]
fn params_should_work() {
	new_test_ext().execute_with(|| {
		assert_eq!(Democracy::referendum_count(0), 0);
		assert_eq!(Balances::free_balance(Public::from_string("/Alice_stash").ok().unwrap()), 0);
		assert_eq!(Balances::total_issuance(), 210);
	});
}

fn set_balance_proposal(value: u128) -> BoundedCallOf<Test> {
	let inner = pallet_balances::Call::set_balance {
		who: Public::from_string("/Alice_stash").ok().unwrap(),
		new_free: value,
		new_reserved: 0,
	};
	let outer = RuntimeCall::Balances(inner);
	Preimage::bound(outer).unwrap()
}

#[test]
fn set_balance_proposal_is_correctly_filtered_out() {
	for i in 0..10 {
		let call = Preimage::realize(&set_balance_proposal(i)).unwrap().0;
		assert!(!<Test as frame_system::Config>::BaseCallFilter::contains(&call));
	}
}

fn create_dao(who: Public) -> DispatchResult {
	let dao = serde_json::to_vec(&get_dao_json()).ok().unwrap();

	Dao::create_dao(RuntimeOrigin::signed(who), vec![], vec![], dao)
}

fn init_dao_token_accounts(dao_id: DaoId) -> DispatchResult {
	let dao_account_id = Dao::dao_account_id(dao_id);

	Assets::transfer(
		RuntimeOrigin::signed(dao_account_id),
		codec::Compact(0),
		Public::from_string("/Alice").ok().unwrap(),
		10,
	)?;
	Assets::transfer(
		RuntimeOrigin::signed(dao_account_id),
		codec::Compact(0),
		Public::from_string("/Bob").ok().unwrap(),
		20,
	)?;
	Assets::transfer(
		RuntimeOrigin::signed(dao_account_id),
		codec::Compact(0),
		Public::from_string("/Charlie").ok().unwrap(),
		30,
	)?;
	Assets::transfer(
		RuntimeOrigin::signed(dao_account_id),
		codec::Compact(0),
		Public::from_string("/Dave").ok().unwrap(),
		40,
	)?;
	Assets::transfer(
		RuntimeOrigin::signed(dao_account_id),
		codec::Compact(0),
		Public::from_string("/Eve").ok().unwrap(),
		50,
	)?;
	Assets::transfer(
		RuntimeOrigin::signed(dao_account_id),
		codec::Compact(0),
		Public::from_string("/Ferdie").ok().unwrap(),
		60,
	)?;

	Ok(())
}

fn propose_set_balance(
	dao_id: DaoId,
	who: Public,
	value: u128,
	delay: u128,
) -> DispatchResultWithPostInfo {
	Democracy::propose(RuntimeOrigin::signed(who), dao_id, set_balance_proposal(value), delay)
}

fn next_block() {
	System::set_block_number(System::block_number() + 1);
	Scheduler::on_initialize(System::block_number());
}

fn fast_forward_to(n: u64) {
	while System::block_number() < n {
		next_block();
	}
}

fn begin_referendum() -> ReferendumIndex {
	let alice = Public::from_string("/Alice").ok().unwrap();

	System::set_block_number(0);

	assert_ok!(create_dao(alice));
	assert_ok!(init_dao_token_accounts(0));
	assert_ok!(propose_set_balance(0, alice, 2, 1));

	fast_forward_to(3);

	0
}

fn aye(who: Public) -> AccountVote<u128> {
	AccountVote::Standard { vote: AYE, balance: Balances::free_balance(&who) }
}

fn nay(who: Public) -> AccountVote<u128> {
	AccountVote::Standard { vote: NAY, balance: Balances::free_balance(&who) }
}

fn big_aye(who: Public) -> AccountVote<u128> {
	AccountVote::Standard { vote: BIG_AYE, balance: Balances::free_balance(&who) }
}

fn big_nay(who: Public) -> AccountVote<u128> {
	AccountVote::Standard { vote: BIG_NAY, balance: Balances::free_balance(&who) }
}

fn tally(dao_id: DaoId, r: ReferendumIndex) -> Tally<u128> {
	Democracy::referendum_status(dao_id, r).unwrap().tally
}

pub fn get_dao_json() -> Value {
	json!({
		"name": "name",
		"purpose": "purpose",
		"metadata": "metadata",
		"policy": {
			"proposal_period": 100,
			"governance": {
				"GovernanceV1": {
					"enactment_period": 2,
					"launch_period": 2,
					"voting_period": 2,
					"vote_locking_period": 3,
					"fast_track_voting_period": 3,
					"cooloff_period": 2,
					"minimum_deposit": 1,
					"instant_allowed": true
				}
			}
		},
		"token": {
			"token_id": 0,
			"initial_balance": "210",
			"metadata": {
				"name": "name",
				"symbol": "symbol",
				"decimals": 3
			}
		}
	})
}
