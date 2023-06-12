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

//! The tests for functionality concerning normal starting, ending and enacting of referenda.

use super::*;

#[test]
fn simple_passing_should_work() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();

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
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(alice), 0, r, aye(alice)));
		assert_eq!(tally(0, r), Tally { ayes: 1, nays: 0, turnout: 10 });
		assert_eq!(Democracy::lowest_unbaked(0), 0);
		next_block();
		next_block();
		assert_eq!(Democracy::lowest_unbaked(0), 1);

		next_block();
		assert_eq!(Balances::free_balance(Public::from_string("/Alice_stash").ok().unwrap()), 2);
	});
}

#[test]
fn simple_failing_should_work() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();

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
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(alice), 0, r, nay(alice)));
		assert_eq!(tally(0, r), Tally { ayes: 0, nays: 1, turnout: 10 });

		next_block();
		next_block();

		assert_eq!(Balances::free_balance(Public::from_string("/Alice_stash").ok().unwrap()), 0);
	});
}

#[test]
fn ooo_inject_referendums_should_work() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();

		assert_ok!(create_dao(alice));
		assert_ok!(init_dao_token_accounts(0));

		let r1 = Democracy::inject_referendum(
			0,
			3,
			0,
			set_balance_proposal(3),
			VoteThreshold::SuperMajorityApprove,
			0,
		);
		let r2 = Democracy::inject_referendum(
			0,
			2,
			0,
			set_balance_proposal(2),
			VoteThreshold::SuperMajorityApprove,
			0,
		);

		assert_ok!(Democracy::vote(RuntimeOrigin::signed(alice), 0, r2, aye(alice)));
		assert_eq!(tally(0, r2), Tally { ayes: 1, nays: 0, turnout: 10 });

		next_block();

		assert_ok!(Democracy::vote(RuntimeOrigin::signed(alice), 0, r1, aye(alice)));
		assert_eq!(tally(0, r1), Tally { ayes: 1, nays: 0, turnout: 10 });

		next_block();
		next_block();
		assert_eq!(Balances::free_balance(Public::from_string("/Alice_stash").ok().unwrap()), 2);

		next_block();
		next_block();
		// TODO
		// assert_eq!(Balances::free_balance(Public::from_string("/Alice_stash").ok().unwrap()), 3);
	});
}

#[test]
fn delayed_enactment_should_work() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let bob = Public::from_string("/Bob").ok().unwrap();
		let charlie = Public::from_string("/Charlie").ok().unwrap();
		let dave = Public::from_string("/Dave").ok().unwrap();
		let eve = Public::from_string("/Eve").ok().unwrap();
		let ferdie = Public::from_string("/Ferdie").ok().unwrap();

		assert_ok!(create_dao(alice));
		assert_ok!(init_dao_token_accounts(0));

		let r = Democracy::inject_referendum(
			0,
			2,
			0,
			set_balance_proposal(2),
			VoteThreshold::SuperMajorityApprove,
			1,
		);
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(alice), 0, r, aye(alice)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(bob), 0, r, aye(bob)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(charlie), 0, r, aye(charlie)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(dave), 0, r, aye(dave)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(eve), 0, r, aye(eve)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(ferdie), 0, r, aye(ferdie)));

		assert_eq!(tally(0, r), Tally { ayes: 21, nays: 0, turnout: 210 });

		next_block();
		next_block();
		assert_eq!(Balances::free_balance(Public::from_string("/Alice_stash").ok().unwrap()), 0);

		next_block();
		next_block();
		assert_eq!(Balances::free_balance(Public::from_string("/Alice_stash").ok().unwrap()), 2);
	});
}

#[test]
fn lowest_unbaked_should_be_sensible() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();

		assert_ok!(create_dao(alice));
		assert_ok!(init_dao_token_accounts(0));

		let r1 = Democracy::inject_referendum(
			0,
			3,
			0,
			set_balance_proposal(1),
			VoteThreshold::SuperMajorityApprove,
			0,
		);
		let r2 = Democracy::inject_referendum(
			0,
			2,
			0,
			set_balance_proposal(2),
			VoteThreshold::SuperMajorityApprove,
			0,
		);
		let r3 = Democracy::inject_referendum(
			0,
			10,
			0,
			set_balance_proposal(3),
			VoteThreshold::SuperMajorityApprove,
			0,
		);
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(alice), 0, r1, aye(alice)));
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(alice), 0, r2, aye(alice)));
		// r3 is canceled
		assert_ok!(Democracy::cancel_referendum(RuntimeOrigin::root(), 0, r3.into()));
		assert_eq!(Democracy::lowest_unbaked(0), 0);

		next_block();
		// r2 ends with approval
		assert_eq!(Democracy::lowest_unbaked(0), 0);

		next_block();
		// r1 ends with approval
		assert_eq!(Democracy::lowest_unbaked(0), 3);
		assert_eq!(Democracy::lowest_unbaked(0), Democracy::referendum_count(0));

		next_block();
		// r2 is executed
		assert_eq!(Balances::free_balance(Public::from_string("/Alice_stash").ok().unwrap()), 2);

		next_block();
		// next_block();
		// next_block();
		// r1 is executed
		// assert_eq!(Balances::free_balance(Public::from_string("/Alice_stash").ok().unwrap()), 1);
	});
}
