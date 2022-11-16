// This file is part of Substrate.

// Copyright (C) 2020-2022 Parity Technologies (UK) Ltd.
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

// Changes made comparing to the original tests of the FRAME pallet-treasure:
// - using DAO as a parameter for pallet functions

//! DAO Treasury pallet tests.

#![cfg(test)]

use std::cell::RefCell;

use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BadOrigin, BlakeTwo256, IdentityLookup},
};

use frame_support::{
	assert_noop, assert_ok,
	dispatch::DispatchError,
	parameter_types,
	traits::{AsEnsureOriginWithArg, ConstU32, ConstU64, OnInitialize},
	PalletId,
};

use super::*;
use crate as treasury;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Treasury: treasury::{Pallet, Call, Storage, Event<T>},
	}
);

parameter_types! {
	pub BlockWeights: frame_system::limits::BlockWeights =
		frame_system::limits::BlockWeights::simple_max(1024);
}
impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = u64;
	type Call = Call;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u128; // u64 is not enough to hold bytes used to generate bounty account
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u64>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}
impl pallet_balances::Config for Test {
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type Balance = u64;
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU64<1>;
	type AccountStore = System;
	type WeightInfo = ();
}
thread_local! {
	static TEN_TO_FOURTEEN: RefCell<Vec<u128>> = RefCell::new(vec![10,11,12,13,14]);
}
parameter_types! {
	pub const ProposalBond: Permill = Permill::from_percent(5);
	pub const Burn: Permill = Permill::from_percent(50);
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
	pub const DaoPalletId: PalletId = PalletId(*b"py/sctld");
}
pub struct TestSpendOrigin;
impl frame_support::traits::EnsureOrigin<Origin> for TestSpendOrigin {
	type Success = u64;
	fn try_origin(o: Origin) -> Result<Self::Success, Origin> {
		Result::<frame_system::RawOrigin<_>, Origin>::from(o).and_then(|o| match o {
			frame_system::RawOrigin::Root => Ok(u64::max_value()),
			frame_system::RawOrigin::Signed(10) => Ok(5),
			frame_system::RawOrigin::Signed(11) => Ok(10),
			frame_system::RawOrigin::Signed(12) => Ok(20),
			frame_system::RawOrigin::Signed(13) => Ok(50),
			r => Err(Origin::from(r)),
		})
	}
	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin() -> Result<Origin, ()> {
		Ok(Origin::root())
	}
}

pub struct TestDaoProvider;
impl DaoProvider for TestDaoProvider {
	type Id = u32;
	type AccountId = u128;
	type Policy = DaoPolicy;

	fn exists(_id: Self::Id) -> Result<(), DispatchError> {
		Ok(())
	}

	fn count() -> u32 {
		1
	}

	fn policy(_id: Self::Id) -> Result<Self::Policy, DispatchError> {
		Ok(DaoPolicy {
			proposal_bond: 5,
			proposal_bond_min: 0,
			proposal_bond_max: None,
			proposal_period: 100,
			approve_origin: (3, 5),
			reject_origin: (1, 2),
		})
	}

	fn dao_account_id(id: Self::Id) -> Self::AccountId {
		PalletId(*b"py/sctld").into_sub_account_truncating(id)
	}
}

impl Config for Test {
	type PalletId = TreasuryPalletId;
	type Currency = pallet_balances::Pallet<Test>;
	type ApproveOrigin = AsEnsureOriginWithArg<frame_system::EnsureRoot<u128>>;
	type RejectOrigin = AsEnsureOriginWithArg<frame_system::EnsureRoot<u128>>;
	type Event = Event;
	type OnSlash = ();
	type SpendPeriod = ConstU64<2>;
	type Burn = Burn;
	type BurnDestination = (); // Just gets burned.
	type WeightInfo = ();
	type SpendFunds = ();
	type MaxApprovals = ConstU32<100>;
	type SpendOrigin = TestSpendOrigin;
	type DaoProvider = TestDaoProvider;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
	pallet_balances::GenesisConfig::<Test> {
		// Total issuance will be 200 with treasury account initialized at ED.
		balances: vec![(0, 100), (1, 98), (2, 1)],
	}
	.assimilate_storage(&mut t)
	.unwrap();
	t.into()
}

#[test]
fn genesis_config_works() {
	new_test_ext().execute_with(|| {
		assert_eq!(Treasury::pot(0), 0);
		assert_eq!(Treasury::proposal_count(0), 0);
	});
}

#[test]
fn spend_origin_permissioning_works() {
	new_test_ext().execute_with(|| {
		assert_noop!(Treasury::spend(Origin::signed(1), 0, 1, 1), BadOrigin);
		assert_noop!(
			Treasury::spend(Origin::signed(10), 0, 6, 1),
			Error::<Test>::InsufficientPermission
		);
		assert_noop!(
			Treasury::spend(Origin::signed(11), 0, 11, 1),
			Error::<Test>::InsufficientPermission
		);
		assert_noop!(
			Treasury::spend(Origin::signed(12), 0, 21, 1),
			Error::<Test>::InsufficientPermission
		);
		assert_noop!(
			Treasury::spend(Origin::signed(13), 0, 51, 1),
			Error::<Test>::InsufficientPermission
		);
	});
}

#[test]
fn spend_origin_works() {
	new_test_ext().execute_with(|| {
		// Check that accumulate works when we have Some value in Dummy already.
		Balances::make_free_balance_be(&<Test as Config>::DaoProvider::dao_account_id(0), 101);
		assert_ok!(Treasury::spend(Origin::signed(10), 0, 5, 6));
		assert_ok!(Treasury::spend(Origin::signed(10), 0, 5, 6));
		assert_ok!(Treasury::spend(Origin::signed(10), 0, 5, 6));
		assert_ok!(Treasury::spend(Origin::signed(10), 0, 5, 6));
		assert_ok!(Treasury::spend(Origin::signed(11), 0, 10, 6));
		assert_ok!(Treasury::spend(Origin::signed(12), 0, 20, 6));
		assert_ok!(Treasury::spend(Origin::signed(13), 0, 50, 6));

		<Treasury as OnInitialize<u64>>::on_initialize(1);
		assert_eq!(Balances::free_balance(6), 0);

		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Balances::free_balance(6), 100);
		assert_eq!(Treasury::pot(0), 0);
	});
}

#[test]
fn minting_works() {
	new_test_ext().execute_with(|| {
		// Check that accumulate works when we have Some value in Dummy already.
		Balances::make_free_balance_be(&<Test as Config>::DaoProvider::dao_account_id(0), 101);
		assert_eq!(Treasury::pot(0), 100);
	});
}

#[test]
fn spend_proposal_takes_min_deposit() {
	new_test_ext().execute_with(|| {
		assert_ok!(Treasury::propose_spend(Origin::signed(0), 0, 1, 3));
		assert_eq!(Balances::free_balance(0), 100);
		assert_eq!(Balances::reserved_balance(0), 0);
	});
}

#[test]
fn spend_proposal_takes_proportional_deposit() {
	new_test_ext().execute_with(|| {
		assert_ok!(Treasury::propose_spend(Origin::signed(0), 0, 100, 3));
		assert_eq!(Balances::free_balance(0), 95);
		assert_eq!(Balances::reserved_balance(0), 5);
	});
}

#[test]
fn spend_proposal_fails_when_proposer_poor() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Treasury::propose_spend(Origin::signed(2), 0, 100, 3),
			Error::<Test, _>::InsufficientProposersBalance,
		);
	});
}

#[test]
fn accepted_spend_proposal_ignored_outside_spend_period() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&<Test as Config>::DaoProvider::dao_account_id(0), 101);

		assert_ok!(Treasury::propose_spend(Origin::signed(0), 0, 100, 3));
		assert_ok!(Treasury::approve_proposal(Origin::root(), 0, 0));

		<Treasury as OnInitialize<u64>>::on_initialize(1);
		assert_eq!(Balances::free_balance(3), 0);
		assert_eq!(Treasury::pot(0), 100);
	});
}

#[test]
fn unused_pot_should_diminish() {
	new_test_ext().execute_with(|| {
		let init_total_issuance = Balances::total_issuance();
		Balances::make_free_balance_be(&<Test as Config>::DaoProvider::dao_account_id(0), 101);
		assert_eq!(Balances::total_issuance(), init_total_issuance + 101);

		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Treasury::pot(0), 50);
		assert_eq!(Balances::total_issuance(), init_total_issuance + 51);
	});
}

#[test]
fn rejected_spend_proposal_ignored_on_spend_period() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&<Test as Config>::DaoProvider::dao_account_id(0), 101);

		assert_ok!(Treasury::propose_spend(Origin::signed(0), 0, 100, 3));
		assert_ok!(Treasury::reject_proposal(Origin::root(), 0, 0));

		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Balances::free_balance(3), 0);
		assert_eq!(Treasury::pot(0), 50);
	});
}

#[test]
fn reject_already_rejected_spend_proposal_fails() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&<Test as Config>::DaoProvider::dao_account_id(0), 101);

		assert_ok!(Treasury::propose_spend(Origin::signed(0), 0, 100, 3));
		assert_ok!(Treasury::reject_proposal(Origin::root(), 0, 0));
		assert_noop!(
			Treasury::reject_proposal(Origin::root(), 0, 0),
			Error::<Test, _>::InvalidIndex
		);
	});
}

#[test]
fn reject_non_existent_spend_proposal_fails() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Treasury::reject_proposal(Origin::root(), 0, 0),
			Error::<Test, _>::InvalidIndex
		);
	});
}

#[test]
fn accept_non_existent_spend_proposal_fails() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			Treasury::approve_proposal(Origin::root(), 0, 0),
			Error::<Test, _>::InvalidIndex
		);
	});
}

#[test]
fn accept_already_rejected_spend_proposal_fails() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&<Test as Config>::DaoProvider::dao_account_id(0), 101);

		assert_ok!(Treasury::propose_spend(Origin::signed(0), 0, 100, 3));
		assert_ok!(Treasury::reject_proposal(Origin::root(), 0, 0));
		assert_noop!(
			Treasury::approve_proposal(Origin::root(), 0, 0),
			Error::<Test, _>::InvalidIndex
		);
	});
}

#[test]
fn accepted_spend_proposal_enacted_on_spend_period() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&<Test as Config>::DaoProvider::dao_account_id(0), 101);
		assert_eq!(Treasury::pot(0), 100);

		assert_ok!(Treasury::propose_spend(Origin::signed(0), 0, 100, 3));
		assert_ok!(Treasury::approve_proposal(Origin::root(), 0, 0));

		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Balances::free_balance(3), 100);
		assert_eq!(Treasury::pot(0), 0);
	});
}

#[test]
fn pot_underflow_should_not_diminish() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&<Test as Config>::DaoProvider::dao_account_id(0), 101);
		assert_eq!(Treasury::pot(0), 100);

		assert_ok!(Treasury::propose_spend(Origin::signed(0), 0, 150, 3));
		assert_ok!(Treasury::approve_proposal(Origin::root(), 0, 0));

		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Treasury::pot(0), 100); // Pot hasn't changed

		let _ =
			Balances::deposit_into_existing(&<Test as Config>::DaoProvider::dao_account_id(0), 100)
				.unwrap();
		<Treasury as OnInitialize<u64>>::on_initialize(4);
		assert_eq!(Balances::free_balance(3), 150); // Fund has been spent
		assert_eq!(Treasury::pot(0), 25); // Pot has finally changed
	});
}

// Treasury account doesn't get deleted if amount approved to spend is all its free balance.
// i.e. pot should not include existential deposit needed for account survival.
#[test]
fn treasury_account_doesnt_get_deleted() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&<Test as Config>::DaoProvider::dao_account_id(0), 101);
		assert_eq!(Treasury::pot(0), 100);
		let treasury_balance =
			Balances::free_balance(&<Test as Config>::DaoProvider::dao_account_id(0));

		assert_ok!(Treasury::propose_spend(Origin::signed(0), 0, treasury_balance, 3));
		assert_ok!(Treasury::approve_proposal(Origin::root(), 0, 0));

		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Treasury::pot(0), 100); // Pot hasn't changed

		assert_ok!(Treasury::propose_spend(Origin::signed(0), 0, Treasury::pot(0), 3));
		assert_ok!(Treasury::approve_proposal(Origin::root(), 0, 1));

		<Treasury as OnInitialize<u64>>::on_initialize(4);
		assert_eq!(Treasury::pot(0), 0); // Pot is emptied
		assert_eq!(Balances::free_balance(<Test as Config>::DaoProvider::dao_account_id(0)), 1); // but the account is still there
	});
}

// In case treasury account is not existing then it works fine.
// This is useful for chain that will just update runtime.
#[test]
fn inexistent_account_works() {
	let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
	pallet_balances::GenesisConfig::<Test> { balances: vec![(0, 100), (1, 99), (2, 1)] }
		.assimilate_storage(&mut t)
		.unwrap();
	// Treasury genesis config is not build thus treasury account does not exist
	let mut t: sp_io::TestExternalities = t.into();

	t.execute_with(|| {
		assert_eq!(Balances::free_balance(<Test as Config>::DaoProvider::dao_account_id(0)), 0); // Account does not exist
		assert_eq!(Treasury::pot(0), 0); // Pot is empty

		assert_ok!(Treasury::propose_spend(Origin::signed(0), 0, 99, 3));
		assert_ok!(Treasury::approve_proposal(Origin::root(), 0, 0));
		assert_ok!(Treasury::propose_spend(Origin::signed(0), 0, 1, 3));
		assert_ok!(Treasury::approve_proposal(Origin::root(), 0, 1));
		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Treasury::pot(0), 0); // Pot hasn't changed
		assert_eq!(Balances::free_balance(3), 0); // Balance of `3` hasn't changed

		Balances::make_free_balance_be(&<Test as Config>::DaoProvider::dao_account_id(0), 100);
		assert_eq!(Treasury::pot(0), 99); // Pot now contains funds
		assert_eq!(Balances::free_balance(<Test as Config>::DaoProvider::dao_account_id(0)), 100); // Account does exist

		<Treasury as OnInitialize<u64>>::on_initialize(4);

		assert_eq!(Treasury::pot(0), 0); // Pot has changed
		assert_eq!(Balances::free_balance(3), 99); // Balance of `3` has changed
	});
}

#[test]
fn genesis_funding_works() {
	let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
	let initial_funding = 100;
	pallet_balances::GenesisConfig::<Test> {
		// Total issuance will be 200 with treasury account initialized with 100.
		balances: vec![
			(0, 100),
			(<Test as Config>::DaoProvider::dao_account_id(0), initial_funding),
		],
	}
	.assimilate_storage(&mut t)
	.unwrap();
	let mut t: sp_io::TestExternalities = t.into();

	t.execute_with(|| {
		assert_eq!(
			Balances::free_balance(<Test as Config>::DaoProvider::dao_account_id(0)),
			initial_funding
		);
		assert_eq!(Treasury::pot(0), initial_funding - Balances::minimum_balance());
	});
}

#[test]
fn max_approvals_limited() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&<Test as Config>::DaoProvider::dao_account_id(0), u64::MAX);
		Balances::make_free_balance_be(&0, u64::MAX);

		for _ in 0..<Test as Config>::MaxApprovals::get() {
			assert_ok!(Treasury::propose_spend(Origin::signed(0), 0, 100, 3));
			assert_ok!(Treasury::approve_proposal(Origin::root(), 0, 0));
		}

		// One too many will fail
		assert_ok!(Treasury::propose_spend(Origin::signed(0), 0, 100, 3));
		assert_noop!(
			Treasury::approve_proposal(Origin::root(), 0, 0),
			Error::<Test, _>::TooManyApprovals
		);
	});
}

#[test]
fn remove_already_removed_approval_fails() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&<Test as Config>::DaoProvider::dao_account_id(0), 101);

		assert_ok!(Treasury::propose_spend(Origin::signed(0), 0, 100, 3));
		assert_ok!(Treasury::approve_proposal(Origin::root(), 0, 0));
		assert_eq!(Treasury::approvals(0), vec![0]);
		assert_ok!(Treasury::remove_approval(Origin::root(), 0, 0));
		assert_eq!(Treasury::approvals(0), vec![]);

		assert_noop!(
			Treasury::remove_approval(Origin::root(), 0, 0),
			Error::<Test, _>::ProposalNotApproved
		);
	});
}
