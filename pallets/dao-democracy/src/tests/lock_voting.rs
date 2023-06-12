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

//! The tests for functionality concerning locking and lock-voting.

use super::*;
use pallet_dao_assets::AssetBalanceLock;

fn aye(x: u8, balance: u128) -> AccountVote<u128> {
	AccountVote::Standard {
		vote: Vote { aye: true, conviction: Conviction::try_from(x).unwrap() },
		balance,
	}
}

fn nay(x: u8, balance: u128) -> AccountVote<u128> {
	AccountVote::Standard {
		vote: Vote { aye: false, conviction: Conviction::try_from(x).unwrap() },
		balance,
	}
}

fn the_lock(amount: u128) -> AssetBalanceLock<u128> {
	AssetBalanceLock { id: DEMOCRACY_ID, amount }
}

#[test]
fn lock_voting_should_work() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let bob = Public::from_string("/Bob").ok().unwrap();
		let charlie = Public::from_string("/Charlie").ok().unwrap();
		let dave = Public::from_string("/Dave").ok().unwrap();
		let eve = Public::from_string("/Eve").ok().unwrap();

		System::set_block_number(0);

		assert_ok!(create_dao(alice));
		assert_ok!(init_dao_token_accounts(0));

		let r = Democracy::inject_referendum(
			0,
			2,
			0,
			set_balance_proposal(2),
			VoteThreshold::SuperMajorityApprove,
			0,
		);
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(alice), 0, r, nay(5, 10)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(bob), 0, r, aye(4, 20)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(charlie), 0, r, aye(3, 30)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(dave), 0, r, aye(2, 40)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(eve), 0, r, nay(1, 50)));
		assert_eq!(tally(0, r), Tally { ayes: 250, nays: 100, turnout: 150 });

		// All balances are currently locked.
		assert_eq!(Assets::locks(0, alice), vec![the_lock(10)]);
		assert_eq!(Assets::locks(0, bob), vec![the_lock(20)]);
		assert_eq!(Assets::locks(0, charlie), vec![the_lock(30)]);
		assert_eq!(Assets::locks(0, dave), vec![the_lock(40)]);
		assert_eq!(Assets::locks(0, eve), vec![the_lock(50)]);

		fast_forward_to(3);

		// Referendum passed; 1 and 5 didn't get their way and can now reap and unlock.
		assert_ok!(Democracy::remove_vote(RuntimeOrigin::signed(alice), 0, r));
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(alice), 0, alice));
		// Anyone can reap and unlock anyone else's in this context.
		assert_ok!(Democracy::remove_other_vote(RuntimeOrigin::signed(bob), 0, eve, r));
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(bob), 0, eve));

		// 2, 3, 4 got their way with the vote, so they cannot be reaped by others.
		assert_noop!(
			Democracy::remove_other_vote(RuntimeOrigin::signed(alice), 0, bob, r),
			Error::<Test>::NoPermission
		);
		// However, they can be unvoted by the owner, though it will make no difference to the lock.
		assert_ok!(Democracy::remove_vote(RuntimeOrigin::signed(bob), 0, r));
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(bob), 0, bob));

		assert_eq!(Assets::locks(0, alice), vec![]);
		assert_eq!(Assets::locks(0, bob), vec![the_lock(20)]);
		assert_eq!(Assets::locks(0, charlie), vec![the_lock(30)]);
		assert_eq!(Assets::locks(0, dave), vec![the_lock(40)]);
		assert_eq!(Assets::locks(0, eve), vec![]);
		// assert_eq!(Assets::balance(42), 2);

		fast_forward_to(7);
		// No change yet...
		assert_noop!(
			Democracy::remove_other_vote(RuntimeOrigin::signed(alice), 0, dave, r),
			Error::<Test>::NoPermission
		);
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(alice), 0, dave));
		assert_eq!(Assets::locks(0, dave), vec![the_lock(40)]);
		fast_forward_to(8);
		// 4 should now be able to reap and unlock
		assert_ok!(Democracy::remove_other_vote(RuntimeOrigin::signed(alice), 0, dave, r));
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(alice), 0, dave));
		assert_eq!(Assets::locks(0, dave), vec![]);

		fast_forward_to(13);
		assert_noop!(
			Democracy::remove_other_vote(RuntimeOrigin::signed(alice), 0, charlie, r),
			Error::<Test>::NoPermission
		);
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(alice), 0, charlie));
		assert_eq!(Assets::locks(0, charlie), vec![the_lock(30)]);
		fast_forward_to(14);
		assert_ok!(Democracy::remove_other_vote(RuntimeOrigin::signed(alice), 0, charlie, r));
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(alice), 0, charlie));
		assert_eq!(Assets::locks(0, charlie), vec![]);

		// 2 doesn't need to reap_vote here because it was already done before.
		fast_forward_to(25);
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(alice), 0, bob));
		assert_eq!(Assets::locks(0, bob), vec![the_lock(20)]);
		fast_forward_to(26);
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(alice), 0, bob));
		assert_eq!(Assets::locks(0, bob), vec![]);
	});
}

#[test]
fn no_locks_without_conviction_should_work() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let bob = Public::from_string("/Bob").ok().unwrap();
		let eve = Public::from_string("/Eve").ok().unwrap();

		System::set_block_number(0);

		assert_ok!(create_dao(alice));
		assert_ok!(init_dao_token_accounts(0));

		assert_ok!(Democracy::delegate(
			RuntimeOrigin::signed(eve),
			0,
			alice,
			Conviction::Locked5x,
			5
		));
		assert_ok!(Democracy::undelegate(RuntimeOrigin::signed(eve), 0));
		// locked 5 until 16 * 3 = #48

		let r = Democracy::inject_referendum(
			0,
			2,
			0,
			set_balance_proposal(2),
			VoteThreshold::SuperMajorityApprove,
			0,
		);
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(alice), 0, r, aye(0, 10)));

		fast_forward_to(3);

		// TODO
		// assert_eq!(Balances::free_balance(Public::from_string("/Alice_Stash").ok().unwrap()), 2);
		assert_ok!(Democracy::remove_other_vote(RuntimeOrigin::signed(bob), 0, alice, r));
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(bob), 0, alice));
		assert_eq!(Assets::locks(0, alice), vec![]);
	});
}

#[test]
fn lock_voting_should_work_with_delegation() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let bob = Public::from_string("/Bob").ok().unwrap();
		let charlie = Public::from_string("/Charlie").ok().unwrap();
		let dave = Public::from_string("/Dave").ok().unwrap();
		let eve = Public::from_string("/Eve").ok().unwrap();

		System::set_block_number(0);

		assert_ok!(create_dao(alice));
		assert_ok!(init_dao_token_accounts(0));

		let r = Democracy::inject_referendum(
			0,
			2,
			0,
			set_balance_proposal(2),
			VoteThreshold::SuperMajorityApprove,
			0,
		);
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(alice), 0, r, nay(5, 10)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(bob), 0, r, aye(4, 20)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(charlie), 0, r, aye(3, 30)));
		assert_ok!(Democracy::delegate(
			RuntimeOrigin::signed(dave),
			0,
			bob,
			Conviction::Locked2x,
			40
		));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(eve), 0, r, nay(1, 50)));

		assert_eq!(tally(0, r), Tally { ayes: 250, nays: 100, turnout: 150 });

		next_block();
		next_block();

		// TODO
		// assert_eq!(Balances::free_balance(Public::from_string("/Alice_Stash").ok().unwrap()), 2);
	});
}

fn setup_three_referenda() -> (u32, u32, u32) {
	let eve = Public::from_string("/Eve").ok().unwrap();

	let r1 = Democracy::inject_referendum(
		0,
		2,
		0,
		set_balance_proposal(2),
		VoteThreshold::SimpleMajority,
		0,
	);
	assert_ok!(Democracy::vote(RuntimeOrigin::signed(eve), 0, r1, aye(4, 10)));

	let r2 = Democracy::inject_referendum(
		0,
		2,
		0,
		set_balance_proposal(2),
		VoteThreshold::SimpleMajority,
		0,
	);
	assert_ok!(Democracy::vote(RuntimeOrigin::signed(eve), 0, r2, aye(3, 20)));

	let r3 = Democracy::inject_referendum(
		0,
		2,
		0,
		set_balance_proposal(2),
		VoteThreshold::SimpleMajority,
		0,
	);
	assert_ok!(Democracy::vote(RuntimeOrigin::signed(eve), 0, r3, aye(2, 50)));

	fast_forward_to(2);

	(r1, r2, r3)
}

#[test]
fn prior_lockvotes_should_be_enforced() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let eve = Public::from_string("/Eve").ok().unwrap();

		System::set_block_number(0);

		assert_ok!(create_dao(alice));
		assert_ok!(init_dao_token_accounts(0));

		let r = setup_three_referenda();
		// r.0 locked 10 until 2 + 8 * 3 = #26
		// r.1 locked 20 until 2 + 4 * 3 = #14
		// r.2 locked 50 until 2 + 2 * 3 = #8

		fast_forward_to(7);
		assert_noop!(
			Democracy::remove_other_vote(RuntimeOrigin::signed(alice), 0, eve, r.2),
			Error::<Test>::NoPermission
		);
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert_eq!(Assets::locks(0, eve), vec![the_lock(50)]);
		fast_forward_to(8);
		assert_ok!(Democracy::remove_other_vote(RuntimeOrigin::signed(alice), 0, eve, r.2));
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert_eq!(Assets::locks(0, eve), vec![the_lock(20)]);
		fast_forward_to(13);
		assert_noop!(
			Democracy::remove_other_vote(RuntimeOrigin::signed(alice), 0, eve, r.1),
			Error::<Test>::NoPermission
		);
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert_eq!(Assets::locks(0, eve), vec![the_lock(20)]);
		fast_forward_to(14);
		assert_ok!(Democracy::remove_other_vote(RuntimeOrigin::signed(alice), 0, eve, r.1));
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert_eq!(Assets::locks(0, eve), vec![the_lock(10)]);
		fast_forward_to(25);
		assert_noop!(
			Democracy::remove_other_vote(RuntimeOrigin::signed(alice), 0, eve, r.0),
			Error::<Test>::NoPermission
		);
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert_eq!(Assets::locks(0, eve), vec![the_lock(10)]);
		fast_forward_to(26);
		assert_ok!(Democracy::remove_other_vote(RuntimeOrigin::signed(alice), 0, eve, r.0));
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert_eq!(Assets::locks(0, eve), vec![]);
	});
}

#[test]
fn single_consolidation_of_lockvotes_should_work_as_before() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let eve = Public::from_string("/Eve").ok().unwrap();

		System::set_block_number(0);

		assert_ok!(create_dao(alice));
		assert_ok!(init_dao_token_accounts(0));

		let r = setup_three_referenda();
		// r.0 locked 10 until 2 + 8 * 3 = #26
		// r.1 locked 20 until 2 + 4 * 3 = #14
		// r.2 locked 50 until 2 + 2 * 3 = #8

		fast_forward_to(7);
		assert_ok!(Democracy::remove_vote(RuntimeOrigin::signed(eve), 0, r.2));
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert_eq!(Assets::locks(0, eve), vec![the_lock(50)]);
		fast_forward_to(8);
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert_eq!(Assets::locks(0, eve), vec![the_lock(20)]);

		fast_forward_to(13);
		assert_ok!(Democracy::remove_vote(RuntimeOrigin::signed(eve), 0, r.1));
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert_eq!(Assets::locks(0, eve), vec![the_lock(20)]);
		fast_forward_to(14);
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert_eq!(Assets::locks(0, eve), vec![the_lock(10)]);

		fast_forward_to(25);
		assert_ok!(Democracy::remove_vote(RuntimeOrigin::signed(eve), 0, r.0));
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert_eq!(Assets::locks(0, eve), vec![the_lock(10)]);
		fast_forward_to(26);
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert_eq!(Assets::locks(0, eve), vec![]);
	});
}

#[test]
fn multi_consolidation_of_lockvotes_should_be_conservative() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let eve = Public::from_string("/Eve").ok().unwrap();

		System::set_block_number(0);

		assert_ok!(create_dao(alice));
		assert_ok!(init_dao_token_accounts(0));

		let r = setup_three_referenda();
		// r.0 locked 10 until 2 + 8 * 3 = #26
		// r.1 locked 20 until 2 + 4 * 3 = #14
		// r.2 locked 50 until 2 + 2 * 3 = #8

		assert_ok!(Democracy::remove_vote(RuntimeOrigin::signed(eve), 0, r.2));
		assert_ok!(Democracy::remove_vote(RuntimeOrigin::signed(eve), 0, r.1));
		assert_ok!(Democracy::remove_vote(RuntimeOrigin::signed(eve), 0, r.0));

		fast_forward_to(8);
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert!(Assets::locks(0, eve)[0].amount >= 20);

		fast_forward_to(14);
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert!(Assets::locks(0, eve)[0].amount >= 10);

		fast_forward_to(26);
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert_eq!(Assets::locks(0, eve), vec![]);
	});
}

#[test]
fn locks_should_persist_from_voting_to_delegation() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let eve = Public::from_string("/Eve").ok().unwrap();

		System::set_block_number(0);

		assert_ok!(create_dao(alice));
		assert_ok!(init_dao_token_accounts(0));

		let r = Democracy::inject_referendum(
			0,
			2,
			0,
			set_balance_proposal(2),
			VoteThreshold::SimpleMajority,
			0,
		);
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(eve), 0, r, aye(4, 10)));
		fast_forward_to(2);
		assert_ok!(Democracy::remove_vote(RuntimeOrigin::signed(eve), 0, r));
		// locked 10 until #26.

		assert_ok!(Democracy::delegate(
			RuntimeOrigin::signed(eve),
			0,
			alice,
			Conviction::Locked3x,
			20
		));
		// locked 20.
		assert!(Assets::locks(0, eve)[0].amount == 20);

		assert_ok!(Democracy::undelegate(RuntimeOrigin::signed(eve), 0));
		// locked 20 until #14

		fast_forward_to(13);
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert!(Assets::locks(0, eve)[0].amount == 20);

		fast_forward_to(14);
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert!(Assets::locks(0, eve)[0].amount >= 10);

		fast_forward_to(25);
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert!(Assets::locks(0, eve)[0].amount >= 10);

		fast_forward_to(26);
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert_eq!(Assets::locks(0, eve), vec![]);
	});
}

#[test]
fn locks_should_persist_from_delegation_to_voting() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let eve = Public::from_string("/Eve").ok().unwrap();

		System::set_block_number(0);

		assert_ok!(create_dao(alice));
		assert_ok!(init_dao_token_accounts(0));

		assert_ok!(Democracy::delegate(
			RuntimeOrigin::signed(eve),
			0,
			alice,
			Conviction::Locked5x,
			5
		));
		assert_ok!(Democracy::undelegate(RuntimeOrigin::signed(eve), 0));
		// locked 5 until 16 * 3 = #48

		let r = setup_three_referenda();
		// r.0 locked 10 until 2 + 8 * 3 = #26
		// r.1 locked 20 until 2 + 4 * 3 = #14
		// r.2 locked 50 until 2 + 2 * 3 = #8

		assert_ok!(Democracy::remove_vote(RuntimeOrigin::signed(eve), 0, r.2));
		assert_ok!(Democracy::remove_vote(RuntimeOrigin::signed(eve), 0, r.1));
		assert_ok!(Democracy::remove_vote(RuntimeOrigin::signed(eve), 0, r.0));

		fast_forward_to(8);
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert!(Assets::locks(0, eve)[0].amount >= 20);

		fast_forward_to(14);
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert!(Assets::locks(0, eve)[0].amount >= 10);

		fast_forward_to(26);
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert!(Assets::locks(0, eve)[0].amount >= 5);

		fast_forward_to(48);
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(eve), 0, eve));
		assert_eq!(Assets::locks(0, eve), vec![]);
	});
}
