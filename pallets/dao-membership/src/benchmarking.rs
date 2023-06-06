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
use frame_benchmarking::{account, benchmarks_instance_pallet, whitelist, BenchmarkError};
use frame_support::assert_ok;
use frame_system::RawOrigin;

use dao_primitives::{DaoOrigin, DaoPolicyProportion};
use serde_json::json;

const SEED: u32 = 0;

// Create the pre-requisite information needed to create a dao.
fn setup_dao_payload<T: Config<I>, I: 'static>() -> Vec<u8> {
	let dao_json = json!({
		"name": "name",
		"purpose": "purpose",
		"metadata": "metadata",
		"policy": {
			"proposal_period": 300000
		},
		"token": {
			"token_id": 0,
			"metadata": {
				"name": "token",
				"symbol": "symbol",
				"decimals": 2
			}
		}
	});

	serde_json::to_vec(&dao_json).ok().unwrap()
}

fn create_dao<T: Config<I>, I: 'static>() -> Result<(), DispatchError> {
	let caller = account("caller", 0, SEED);
	let data = setup_dao_payload::<T, I>();

	T::DaoProvider::create_dao(caller, vec![], vec![], data)
}

fn get_dao_origin<T: Config<I>, I: 'static>(
	dao_id: DaoId,
) -> Result<T::RuntimeOrigin, BenchmarkError> {
	let dao_account_id = T::DaoProvider::dao_account_id(dao_id);
	let dao_origin = DaoOrigin {
		dao_account_id: dao_account_id.clone(),
		proportion: DaoPolicyProportion::AtLeast((1, 1)),
	};

	T::DaoProvider::try_successful_origin(&dao_origin).map_err(|_| BenchmarkError::Weightless)
}

fn set_members<T: Config<I>, I: 'static>(origin: OriginFor<T>, members: Vec<T::AccountId>) {
	assert_ok!(<Membership<T, I>>::reset_members(origin, 0, members.clone()));
}

benchmarks_instance_pallet! {
	add_member {
		create_dao::<T, I>()?;
		let members = (1..T::MaxMembers::get() - 1).map(|i| account("member", i, SEED))
			.collect::<Vec<T::AccountId>>();
		let origin = get_dao_origin::<T, I>(0)?;
		set_members::<T, I>(origin.clone(), members);
		let new_member = account::<T::AccountId>("add", T::MaxMembers::get(), SEED);
	}: _<T::RuntimeOrigin>(origin, 0, new_member.clone())
	verify {
		assert!(<Members<T, I>>::get(0).contains(&new_member));
	}

	remove_member {
		create_dao::<T, I>()?;
		let members = (2..T::MaxMembers::get()).map(|i| account("member", i, SEED))
			.collect::<Vec<T::AccountId>>();
		let origin = get_dao_origin::<T, I>(0)?;
		set_members::<T, I>(origin.clone(), members.clone());
		let to_remove = members.first().cloned().unwrap();
	}: _<T::RuntimeOrigin>(origin, 0, to_remove.clone())
	verify {
		assert!(!<Members<T, I>>::get(0).contains(&to_remove));
	}

	swap_member {
		create_dao::<T, I>()?;
		let members = (2..T::MaxMembers::get()).map(|i| account("member", i, SEED))
			.collect::<Vec<T::AccountId>>();
		let origin = get_dao_origin::<T, I>(0)?;
		set_members::<T, I>(origin.clone(), members.clone());
		let add = account::<T::AccountId>("member", 0, SEED);
		let remove = members.first().cloned().unwrap();
	}: _<T::RuntimeOrigin>(origin, 0, remove.clone(), add.clone())
	verify {
		assert!(!<Members<T, I>>::get(0).contains(&remove));
		assert!(<Members<T, I>>::get(0).contains(&add));
	}

	reset_members {
		create_dao::<T, I>()?;
		let m = T::MaxMembers::get();
		let members = (1..m + 1).map(|i| account("member", i, SEED))
			.collect::<Vec<T::AccountId>>();
		let origin = get_dao_origin::<T, I>(0)?;
		set_members::<T, I>(origin.clone(), members.clone());
		let mut new_members = (m..2 * m).map(|i| account("member", i, SEED))
			.collect::<Vec<T::AccountId>>();
	}: _<T::RuntimeOrigin>(origin, 0, new_members.clone())
	verify {
		new_members.sort();
		assert_eq!(<Members<T, I>>::get(0), new_members);
	}

	change_key {
		create_dao::<T, I>()?;
		let members = (1..T::MaxMembers::get()).map(|i| account("member", i, SEED))
			.collect::<Vec<T::AccountId>>();
		let origin = get_dao_origin::<T, I>(0)?;
		set_members::<T, I>(origin.clone(), members.clone());
		let prime = members.last().cloned().unwrap();
		let add = account::<T::AccountId>("member", 0, SEED);
		whitelist!(prime);
	}: _<T::RuntimeOrigin>(RawOrigin::Signed(prime.clone()).into(), 0, add.clone())
	verify {
		assert!(<Members<T, I>>::get(0).contains(&add));
	}

	impl_benchmark_test_suite!(Membership, crate::mock::new_test_ext(), crate::mock::Test);
}
