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

//! The tests for cancelation functionality.

use super::*;

#[test]
fn emergency_cancel_should_work() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let charlie = Public::from_string("/Charlie").ok().unwrap();

		System::set_block_number(0);

		assert_ok!(create_dao(alice));
		assert_ok!(init_dao_token_accounts(0));

		let r = Democracy::inject_referendum(
			0,
			2,
			0,
			set_balance_proposal(2),
			VoteThreshold::SuperMajorityApprove,
			2,
		);
		assert!(Democracy::referendum_status(0, r).is_ok());

		assert_noop!(Democracy::emergency_cancel(RuntimeOrigin::signed(charlie), 0, r), BadOrigin);
		assert_ok!(Democracy::emergency_cancel(RuntimeOrigin::root(), 0, r));
		assert!(Democracy::referendum_info(0, r).is_none());

		// some time later...

		let r = Democracy::inject_referendum(
			0,
			2,
			0,
			set_balance_proposal(2),
			VoteThreshold::SuperMajorityApprove,
			2,
		);
		assert!(Democracy::referendum_status(0, r).is_ok());
		assert_noop!(
			Democracy::emergency_cancel(RuntimeOrigin::root(), 0, r),
			Error::<Test>::AlreadyCanceled,
		);
	});
}
