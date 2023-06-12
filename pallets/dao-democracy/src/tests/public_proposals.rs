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

//! The tests for the public proposal queue.

use super::*;

#[test]
fn backing_for_should_work() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();

		assert_ok!(create_dao(alice));
		assert_ok!(init_dao_token_accounts(0));

		assert_ok!(propose_set_balance(0, alice, 2, 2));
		assert_ok!(propose_set_balance(0, alice, 4, 4));
		assert_ok!(propose_set_balance(0, alice, 3, 3));
		assert_eq!(Democracy::backing_for(0, 0), Some(2));
		assert_eq!(Democracy::backing_for(0, 1), Some(4));
		assert_eq!(Democracy::backing_for(0, 2), Some(3));
	});
}

#[test]
fn deposit_for_proposals_should_be_taken() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let bob = Public::from_string("/Bob").ok().unwrap();
		let eve = Public::from_string("/Eve").ok().unwrap();

		assert_ok!(create_dao(alice));
		assert_ok!(init_dao_token_accounts(0));

		assert_ok!(propose_set_balance(0, alice, 2, 5));
		assert_ok!(Democracy::second(RuntimeOrigin::signed(bob), 0, 0));
		assert_ok!(Democracy::second(RuntimeOrigin::signed(eve), 0, 0));
		assert_ok!(Democracy::second(RuntimeOrigin::signed(eve), 0, 0));
		assert_ok!(Democracy::second(RuntimeOrigin::signed(eve), 0, 0));
		assert_eq!(Assets::balance(0, alice), 5);
		assert_eq!(Assets::balance(0, bob), 15);
		assert_eq!(Assets::balance(0, eve), 35);
	});
}

#[test]
fn deposit_for_proposals_should_be_returned() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let bob = Public::from_string("/Bob").ok().unwrap();
		let eve = Public::from_string("/Eve").ok().unwrap();

		assert_ok!(create_dao(alice));
		assert_ok!(init_dao_token_accounts(0));

		assert_ok!(propose_set_balance(0, alice, 2, 5));
		assert_ok!(Democracy::second(RuntimeOrigin::signed(bob), 0, 0));
		assert_ok!(Democracy::second(RuntimeOrigin::signed(eve), 0, 0));
		assert_ok!(Democracy::second(RuntimeOrigin::signed(eve), 0, 0));
		assert_ok!(Democracy::second(RuntimeOrigin::signed(eve), 0, 0));
		fast_forward_to(3);
		assert_eq!(Assets::balance(0, alice), 10);
		assert_eq!(Assets::balance(0, bob), 20);
		assert_eq!(Assets::balance(0, eve), 50);
	});
}

#[test]
fn proposal_with_deposit_below_minimum_should_not_work() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();

		assert_ok!(create_dao(alice));
		assert_ok!(init_dao_token_accounts(0));

		assert_noop!(propose_set_balance(0, alice, 2, 0), Error::<Test>::ValueLow);
	});
}

#[test]
fn poor_proposer_should_not_work() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();

		assert_ok!(create_dao(alice));
		assert_ok!(init_dao_token_accounts(0));

		assert_noop!(propose_set_balance(0, alice, 2, 11), AssetsError::<Test, _>::BalanceLow);
	});
}

#[test]
fn poor_seconder_should_not_work() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let bob = Public::from_string("/Bob").ok().unwrap();

		assert_ok!(create_dao(alice));
		assert_ok!(init_dao_token_accounts(0));

		assert_ok!(propose_set_balance(0, bob, 2, 11));
		assert_noop!(
			Democracy::second(RuntimeOrigin::signed(alice), 0, 0),
			AssetsError::<Test, _>::BalanceLow
		);
	});
}

#[test]
fn cancel_proposal_should_work() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();

		assert_ok!(create_dao(alice));
		assert_ok!(init_dao_token_accounts(0));

		assert_ok!(propose_set_balance(0, alice, 2, 2));
		assert_ok!(propose_set_balance(0, alice, 4, 4));
		assert_noop!(Democracy::cancel_proposal(RuntimeOrigin::signed(alice), 0, 0), BadOrigin);
		assert_ok!(Democracy::cancel_proposal(RuntimeOrigin::root(), 0, 0));
		System::assert_last_event(
			crate::Event::ProposalCanceled { dao_id: 0, prop_index: 0 }.into(),
		);
		assert_eq!(Democracy::backing_for(0, 0), None);
		assert_eq!(Democracy::backing_for(0, 1), Some(4));
	});
}

#[test]
fn blacklisting_should_work() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();

		System::set_block_number(0);

		assert_ok!(create_dao(alice));
		assert_ok!(init_dao_token_accounts(0));

		let hash = set_balance_proposal(2).hash();

		assert_ok!(propose_set_balance(0, alice, 2, 2));
		assert_ok!(propose_set_balance(0, alice, 4, 4));

		assert_noop!(Democracy::blacklist(RuntimeOrigin::signed(alice), 0, hash, None), BadOrigin);
		assert_ok!(Democracy::blacklist(RuntimeOrigin::root(), 0, hash, None));

		assert_eq!(Democracy::backing_for(0, 0), None);
		assert_eq!(Democracy::backing_for(0, 1), Some(4));

		assert_noop!(propose_set_balance(0, alice, 2, 2), Error::<Test>::ProposalBlacklisted);

		fast_forward_to(2);

		let hash = set_balance_proposal(4).hash();
		assert_ok!(Democracy::referendum_status(0, 0));
		assert_ok!(Democracy::blacklist(RuntimeOrigin::root(), 0, hash, Some(0)));
		assert_noop!(Democracy::referendum_status(0, 0), Error::<Test>::ReferendumInvalid);
	});
}

#[test]
fn runners_up_should_come_after() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let bob = Public::from_string("/Bob").ok().unwrap();

		System::set_block_number(0);

		assert_ok!(create_dao(alice));
		assert_ok!(init_dao_token_accounts(0));

		assert_ok!(propose_set_balance(0, alice, 2, 2));
		assert_ok!(propose_set_balance(0, alice, 4, 4));
		assert_ok!(propose_set_balance(0, alice, 3, 3));
		fast_forward_to(2);
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(bob), 0, 0, aye(bob)));
		fast_forward_to(4);
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(bob), 0, 1, aye(bob)));
		fast_forward_to(6);
		assert_ok!(Democracy::vote(RuntimeOrigin::signed(bob), 0, 2, aye(bob)));
	});
}
