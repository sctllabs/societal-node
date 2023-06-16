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

//! The tests for functionality concerning delegation.

use super::*;

#[test]
fn single_proposal_should_work_with_delegation() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let bob = Public::from_string("/Bob").ok().unwrap();
		let charlie = Public::from_string("/Charlie").ok().unwrap();

		begin_referendum();

		// Delegate first vote.
		assert_ok!(Democracy::delegate(RuntimeOrigin::signed(bob), 0, alice, Conviction::None, 20));
		let r = 0;
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(alice), 0, r, aye(alice)));
		assert_eq!(tally(0, r), Tally { ayes: 3, nays: 0, turnout: 30 });

		// Delegate a second vote.
		assert_ok!(Democracy::delegate(
			RuntimeOrigin::signed(charlie),
			0,
			alice,
			Conviction::None,
			30
		));
		assert_eq!(tally(0, r), Tally { ayes: 6, nays: 0, turnout: 60 });

		// Reduce first vote.
		assert_ok!(Democracy::delegate(RuntimeOrigin::signed(bob), 0, alice, Conviction::None, 10));
		assert_eq!(tally(0, r), Tally { ayes: 5, nays: 0, turnout: 50 });

		// Second vote delegates to first; we don't do tiered delegation, so it doesn't get used.
		assert_ok!(Democracy::delegate(
			RuntimeOrigin::signed(charlie),
			0,
			bob,
			Conviction::None,
			30
		));
		assert_eq!(tally(0, r), Tally { ayes: 2, nays: 0, turnout: 20 });

		// Main voter cancels their vote
		assert_ok!(Democracy::remove_vote(RuntimeOrigin::signed(alice), 0, r));
		assert_eq!(tally(0, r), Tally { ayes: 0, nays: 0, turnout: 0 });

		// First delegator delegates half funds with conviction; nothing changes yet.
		assert_ok!(Democracy::delegate(
			RuntimeOrigin::signed(bob),
			0,
			alice,
			Conviction::Locked1x,
			10
		));
		assert_eq!(tally(0, r), Tally { ayes: 0, nays: 0, turnout: 0 });

		// Main voter reinstates their vote
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(alice), 0, r, aye(alice)));
		assert_eq!(tally(0, r), Tally { ayes: 11, nays: 0, turnout: 20 });
	});
}

#[test]
fn self_delegation_not_allowed() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();

		assert_noop!(
			Democracy::delegate(RuntimeOrigin::signed(alice), 0, alice, Conviction::None, 10),
			Error::<Test>::Nonsense,
		);
	});
}

#[test]
fn cyclic_delegation_should_unwind() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let bob = Public::from_string("/Bob").ok().unwrap();
		let charlie = Public::from_string("/Charlie").ok().unwrap();

		begin_referendum();

		// Check behavior with cycle.
		assert_ok!(Democracy::delegate(RuntimeOrigin::signed(bob), 0, alice, Conviction::None, 20));
		assert_ok!(Democracy::delegate(
			RuntimeOrigin::signed(charlie),
			0,
			bob,
			Conviction::None,
			30
		));
		assert_ok!(Democracy::delegate(
			RuntimeOrigin::signed(alice),
			0,
			charlie,
			Conviction::None,
			10
		));
		let r = 0;
		assert_ok!(Democracy::undelegate(RuntimeOrigin::signed(charlie), 0));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(charlie), 0, r, aye(charlie)));
		assert_ok!(Democracy::undelegate(RuntimeOrigin::signed(alice), 0));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(alice), 0, r, nay(alice)));

		// Delegated vote is counted.
		assert_eq!(tally(0, r), Tally { ayes: 3, nays: 3, turnout: 60 });
	});
}

#[test]
fn single_proposal_should_work_with_vote_and_delegation() {
	// If transactor already voted, delegated vote is overwritten.
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let bob = Public::from_string("/Bob").ok().unwrap();

		begin_referendum();

		let r = 0;
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(alice), 0, r, aye(alice)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(bob), 0, r, nay(bob)));
		assert_eq!(tally(0, r), Tally { ayes: 1, nays: 2, turnout: 30 });

		// Delegate vote.
		assert_ok!(Democracy::remove_vote(RuntimeOrigin::signed(bob), 0, r));
		assert_ok!(Democracy::delegate(RuntimeOrigin::signed(bob), 0, alice, Conviction::None, 20));
		// Delegated vote replaces the explicit vote.
		assert_eq!(tally(0, r), Tally { ayes: 3, nays: 0, turnout: 30 });
	});
}

#[test]
fn single_proposal_should_work_with_undelegation() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let bob = Public::from_string("/Bob").ok().unwrap();

		begin_referendum();

		// Delegate and undelegate vote.
		assert_ok!(Democracy::delegate(RuntimeOrigin::signed(bob), 0, alice, Conviction::None, 20));
		assert_ok!(Democracy::undelegate(RuntimeOrigin::signed(bob), 0));

		fast_forward_to(2);
		let r = 0;
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(alice), 0, r, aye(alice)));

		// Delegated vote is not counted.
		assert_eq!(tally(0, r), Tally { ayes: 1, nays: 0, turnout: 10 });
	});
}

#[test]
fn single_proposal_should_work_with_delegation_and_vote() {
	// If transactor voted, delegated vote is overwritten.
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let bob = Public::from_string("/Bob").ok().unwrap();

		let r = begin_referendum();

		// Delegate, undelegate and vote.
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(alice), 0, r, aye(alice)));
		assert_ok!(Democracy::delegate(RuntimeOrigin::signed(bob), 0, alice, Conviction::None, 20));
		assert_eq!(tally(0, r), Tally { ayes: 3, nays: 0, turnout: 30 });
		assert_ok!(Democracy::undelegate(RuntimeOrigin::signed(bob), 0));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(bob), 0, r, aye(bob)));
		// Delegated vote is not counted.
		assert_eq!(tally(0, r), Tally { ayes: 3, nays: 0, turnout: 30 });
	});
}

#[test]
fn conviction_should_be_honored_in_delegation() {
	// If transactor voted, delegated vote is overwritten.
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let bob = Public::from_string("/Bob").ok().unwrap();

		let r = begin_referendum();

		// Delegate and vote.
		assert_ok!(Democracy::delegate(
			RuntimeOrigin::signed(bob),
			0,
			alice,
			Conviction::Locked6x,
			20
		));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(alice), 0, r, aye(alice)));
		// Delegated vote is huge.
		assert_eq!(tally(0, r), Tally { ayes: 121, nays: 0, turnout: 30 });
	});
}

#[test]
fn split_vote_delegation_should_be_ignored() {
	// If transactor voted, delegated vote is overwritten.
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let bob = Public::from_string("/Bob").ok().unwrap();

		let r = begin_referendum();

		assert_ok!(Democracy::delegate(
			RuntimeOrigin::signed(bob),
			0,
			alice,
			Conviction::Locked6x,
			20
		));
		assert_ok!(Democracy::vote(
			RuntimeOrigin::signed(alice),
			0,
			r,
			AccountVote::Split { aye: 10, nay: 0 }
		));
		// Delegated vote is huge.
		assert_eq!(tally(0, r), Tally { ayes: 1, nays: 0, turnout: 10 });
	});
}

#[test]
fn redelegation_keeps_lock() {
	// If transactor voted, delegated vote is overwritten.
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let bob = Public::from_string("/Bob").ok().unwrap();
		let charlie = Public::from_string("/Charlie").ok().unwrap();

		let r = begin_referendum();

		// Delegate and vote.
		assert_ok!(Democracy::delegate(
			RuntimeOrigin::signed(bob),
			0,
			alice,
			Conviction::Locked6x,
			20
		));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(alice), 0, r, aye(alice)));
		// Delegated vote is huge.
		assert_eq!(tally(0, r), Tally { ayes: 121, nays: 0, turnout: 30 });

		let mut prior_lock = vote::PriorLock::default();

		// Locked balance of delegator exists
		assert_eq!(VotingOf::<Test>::get(0, bob).locked_balance(), 20);
		assert_eq!(VotingOf::<Test>::get(0, bob).prior(), &prior_lock);

		// Delegate someone else at a lower conviction and amount
		assert_ok!(Democracy::delegate(
			RuntimeOrigin::signed(bob),
			0,
			charlie,
			Conviction::None,
			10
		));

		// 6x prior should appear w/ locked balance.
		prior_lock.accumulate(99, 20);
		assert_eq!(VotingOf::<Test>::get(0, bob).prior(), &prior_lock);
		assert_eq!(VotingOf::<Test>::get(0, bob).locked_balance(), 20);
		// Unlock shouldn't work
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(bob), 0, bob));
		assert_eq!(VotingOf::<Test>::get(0, bob).prior(), &prior_lock);
		assert_eq!(VotingOf::<Test>::get(0, bob).locked_balance(), 20);

		fast_forward_to(100);

		// Now unlock can remove the prior lock and reduce the locked amount.
		assert_eq!(VotingOf::<Test>::get(0, bob).prior(), &prior_lock);
		assert_ok!(Democracy::unlock(RuntimeOrigin::signed(bob), 0, bob));
		assert_eq!(VotingOf::<Test>::get(0, bob).prior(), &vote::PriorLock::default());
		assert_eq!(VotingOf::<Test>::get(0, bob).locked_balance(), 10);
	});
}
