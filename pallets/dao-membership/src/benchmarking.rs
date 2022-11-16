// This file is part of Substrate.

// Copyright (C) 2019-2022 Parity Technologies (UK) Ltd.
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

// Changes made comparing to the original benchmarking of the FRAME pallet-membership:
// - using DAO as a parameter for pallet functions

//! DAO Membership pallet benchmarking.

use super::{Pallet as Membership, *};
use frame_benchmarking::{account, benchmarks_instance_pallet, whitelist};
use frame_support::{assert_ok, traits::EnsureOrigin};
use frame_system::RawOrigin;

const SEED: u32 = 0;

fn set_members<T: Config<I>, I: 'static>(members: Vec<T::AccountId>) {
	let approve_origin = T::ApproveOrigin::successful_origin(&(1, 1));

	assert_ok!(<Membership<T, I>>::reset_members(approve_origin.clone(), 0, members.clone()));
}

benchmarks_instance_pallet! {
	add_member {
		let m in 1 .. (T::MaxMembers::get() - 1);

		let members = (0..m).map(|i| account("member", i, SEED)).collect::<Vec<T::AccountId>>();
		set_members::<T, I>(members);
		let new_member = account::<T::AccountId>("add", m, SEED);
	}: {
		assert_ok!(<Membership<T, I>>::add_member(T::ApproveOrigin::successful_origin(&(1, 1)), 0, new_member.clone()));
	}
	verify {
		assert!(<Members<T, I>>::get(0).contains(&new_member));
		#[cfg(test)] crate::mock::clean();
	}

	remove_member {
		let m in 2 .. T::MaxMembers::get();

		let members = (0..m).map(|i| account("member", i, SEED)).collect::<Vec<T::AccountId>>();
		set_members::<T, I>(members.clone());

		let to_remove = members.first().cloned().unwrap();
	}: {
		assert_ok!(<Membership<T, I>>::remove_member(T::ApproveOrigin::successful_origin(&(1, 1)), 0, to_remove.clone()));
	} verify {
		assert!(!<Members<T, I>>::get(0).contains(&to_remove));
		#[cfg(test)] crate::mock::clean();
	}

	swap_member {
		let m in 2 .. T::MaxMembers::get();

		let members = (0..m).map(|i| account("member", i, SEED)).collect::<Vec<T::AccountId>>();
		set_members::<T, I>(members.clone());
		let add = account::<T::AccountId>("member", m, SEED);
		let remove = members.first().cloned().unwrap();
	}: {
		assert_ok!(<Membership<T, I>>::swap_member(
			T::ApproveOrigin::successful_origin(&(1, 1)),
			0,
			remove.clone(),
			add.clone(),
		));
	} verify {
		assert!(!<Members<T, I>>::get(0).contains(&remove));
		assert!(<Members<T, I>>::get(0).contains(&add));
		#[cfg(test)] crate::mock::clean();
	}

	reset_member {
		let m in 1 .. T::MaxMembers::get();

		let members = (1..m+1).map(|i| account("member", i, SEED)).collect::<Vec<T::AccountId>>();
		set_members::<T, I>(members.clone());
		let mut new_members = (m..2*m).map(|i| account("member", i, SEED)).collect::<Vec<T::AccountId>>();
	}: {
		assert_ok!(<Membership<T, I>>::reset_members(T::ApproveOrigin::successful_origin(&(1, 1)), 0, new_members.clone()));
	} verify {
		new_members.sort();
		assert_eq!(<Members<T, I>>::get(0), new_members);
		#[cfg(test)] crate::mock::clean();
	}
	change_key {
		let m in 1 .. T::MaxMembers::get();

		let members = (0..m).map(|i| account("member", i, SEED)).collect::<Vec<T::AccountId>>();
		let prime = members.last().cloned().unwrap();
		set_members::<T, I>(members.clone());

		let add = account::<T::AccountId>("member", m, SEED);
		whitelist!(prime);
	}: {
		assert_ok!(<Membership<T, I>>::change_key(RawOrigin::Signed(prime.clone()).into(), 0, add.clone()));
	} verify {
		assert!(!<Members<T, I>>::get(0).contains(&prime));
		assert!(<Members<T, I>>::get(0).contains(&add));
		#[cfg(test)] crate::mock::clean();
	}

	impl_benchmark_test_suite!(Membership, crate::mock::new_test_ext(), crate::mock::Test);
}
