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

// Changes made comparing to the original benchmarking of the FRAME pallet-collective:
// - using DAO as a parameter for pallet functions

//! DAO Collective pallet benchmarking.

use super::*;
use crate::Pallet as Collective;

use serde_json::json;

use sp_runtime::traits::Bounded;
use sp_std::mem::size_of;

use frame_benchmarking::{account, benchmarks_instance_pallet, whitelisted_caller, BenchmarkError};
use frame_system::{Call as SystemCall, Pallet as System, RawOrigin as SystemOrigin};

const SEED: u32 = 0;

const MAX_BYTES: u32 = 1_024;

// Create the pre-requisite information needed to create a dao.
fn setup_dao_payload<T: Config<I>, I: 'static>() -> Vec<u8> {
	let dao_json = json!({
		"name": "name",
		"purpose": "purpose",
		"metadata": "metadata",
		"policy": {
			"proposal_period": 300000,
			"approve_origin": { "type": "AtLeast", "proportion": [ 1, 2 ] }
		},
		"token": {
			"token_id": 0,
			"initial_balance": "100000000",
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

fn assert_last_event<T: Config<I>, I: 'static>(generic_event: <T as Config<I>>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

fn assert_second_to_last_event<T: Config<I>, I: 'static>(event: <T as Config<I>>::RuntimeEvent) {
	let events = frame_system::Pallet::<T>::events();
	let second_to_last_event = &events.get(events.len() - 2).expect("expected event").event;
	assert_eq!(
		second_to_last_event,
		&event.into(),
		"expected event is not equal to the second to last event {second_to_last_event:?}",
	);
}

benchmarks_instance_pallet! {
	propose_with_meta {
		let b in 1 .. MAX_BYTES;
		let m in 2 .. T::MaxMembers::get();
		let p in 1 .. T::MaxProposals::get();
		let bytes_in_storage = b + size_of::<u32>() as u32;

		create_dao::<T, I>()?;

		// Construct `members`.
		let mut members = vec![];
		for i in 0 .. m - 1 {
			let member = account::<T::AccountId>("member", i, SEED);
			members.push(member);
		}
		let caller: T::AccountId = whitelisted_caller();
		members.push(caller.clone());

		Collective::<T, I>::initialize_members(0, members)?;

		// Add previous proposals.
		for i in 0 .. p - 1 {
			// Proposals should be different so that different proposal hashes are generated
			let proposal: T::Proposal = SystemCall::<T>::remark { remark: vec![i as u8; b as usize] }.into();
			Collective::<T, I>::propose(
				SystemOrigin::Signed(caller.clone()).into(),
				0,
				Box::new(proposal),
				bytes_in_storage,
			)?;
		}

		assert_eq!(Collective::<T, I>::proposals(0).len(), (p - 1) as usize);

		let proposal: T::Proposal = SystemCall::<T>::remark { remark: vec![p as u8; b as usize] }.into();
		let bounded = T::Preimages::bound(proposal.clone())?.transmute();
		let proposal_hash = T::Hashing::hash_of(&bounded);

		let meta: Option<Vec<u8>> = Some("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor \
			incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud \
			exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure \
			dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. \
			Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt \
			mollit anim id est laborum. Lorem ipsum dolor sit amet, consectetur adipiscing elit, \
			sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim \
			veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo \
			consequat. Duis aute irure dolor in reprehenderit".into());
	}: _(
		SystemOrigin::Signed(caller.clone()),
		0,
		Box::new(proposal.clone()),
		bytes_in_storage,
		meta.clone()
	)
	verify {
		// New proposal is recorded
		assert_eq!(Collective::<T, I>::proposals(0).len(), p as usize);

		assert_last_event::<T, I>(
			Event::Proposed {
				dao_id: 0,
				account: caller,
				proposal_index: p - 1,
				proposal_hash,
				proposal,
				threshold: m / 2,
				meta,
			}.into()
		);
	}

	vote {
		// We choose 5 as a minimum so we always trigger a vote in the voting loop (`for j in ...`)
		let m in 5 .. T::MaxMembers::get();

		let p = T::MaxProposals::get();
		let b = MAX_BYTES;
		let bytes_in_storage = b + size_of::<u32>() as u32;

		create_dao::<T, I>()?;

		// Construct `members`.
		let mut members = vec![];
		let proposer: T::AccountId = account::<T::AccountId>("proposer", 0, SEED);
		members.push(proposer.clone());
		for i in 1 .. m - 1 {
			let member = account::<T::AccountId>("member", i, SEED);
			members.push(member);
		}
		let voter: T::AccountId = account::<T::AccountId>("voter", 0, SEED);
		members.push(voter.clone());
		Collective::<T, I>::initialize_members(0, members.clone())?;

		// Add previous proposals
		let mut last_hash = T::Hash::default();
		for i in 0 .. p {
			// Proposals should be different so that different proposal hashes are generated
			let proposal: T::Proposal =
				SystemCall::<T>::remark { remark: vec![i as u8; b as usize] }.into();
			Collective::<T, I>::propose(
				SystemOrigin::Signed(proposer.clone()).into(),
				0,
				Box::new(proposal.clone()),
				bytes_in_storage,
			)?;
			let bounded = T::Preimages::bound(proposal.clone())?.transmute();
			last_hash = T::Hashing::hash_of(&bounded);
		}

		let index = p - 1;
		// Have almost everyone vote aye on last proposal, while keeping it from passing.
		for j in 0 .. m - 3 {
			let voter = &members[j as usize];
			let approve = true;
			Collective::<T, I>::vote(
				SystemOrigin::Signed(voter.clone()).into(),
				0,
				last_hash,
				index,
				approve,
			)?;
		}
		// Voter votes aye without resolving the vote.
		let approve = true;
		Collective::<T, I>::vote(
			SystemOrigin::Signed(voter.clone()).into(),
			0,
			last_hash,
			index,
			approve,
		)?;

		assert_eq!(Collective::<T, I>::proposals(0).len(), p as usize);

		// Voter switches vote to nay, but does not kill the vote, just updates + inserts
		let approve = false;

		// Whitelist voter account from further DB operations.
		let voter_key = frame_system::Account::<T>::hashed_key_for(&voter);
		frame_benchmarking::benchmarking::add_to_whitelist(voter_key.into());
	}: _(SystemOrigin::Signed(voter), 0, last_hash, index, approve)
	verify {
		// All proposals exist and the last proposal has just been updated.
		assert_eq!(Collective::<T, I>::proposals(0).len(), p as usize);
		let voting = Collective::<T, I>::voting(0, &last_hash).ok_or("Proposal Missing")?;
		assert_eq!(voting.ayes.len(), (m - 3) as usize);
		assert_eq!(voting.nays.len(), 1);
	}

	close_early_disapproved {
		// We choose 4 as a minimum so we always trigger a vote in the voting loop (`for j in ...`)
		let m in 4 .. T::MaxMembers::get();
		let p in 1 .. T::MaxProposals::get();

		let bytes = 100;
		let bytes_in_storage = bytes + size_of::<u32>() as u32;

		create_dao::<T, I>()?;

		// Construct `members`.
		let mut members = vec![];
		let proposer = account::<T::AccountId>("proposer", 0, SEED);
		members.push(proposer.clone());
		for i in 1 .. m - 1 {
			let member = account::<T::AccountId>("member", i, SEED);
			members.push(member);
		}
		let voter = account::<T::AccountId>("voter", 0, SEED);
		members.push(voter.clone());
		Collective::<T, I>::initialize_members(0, members.clone())?;

		// Add previous proposals
		let mut last_hash = T::Hash::default();
		for i in 0 .. p {
			// Proposals should be different so that different proposal hashes are generated
			let proposal: T::Proposal = SystemCall::<T>::remark {

				remark: vec![i as u8; bytes as usize]
			}.into();
			Collective::<T, I>::propose(
				SystemOrigin::Signed(proposer.clone()).into(),
				0,
				Box::new(proposal.clone()),
				bytes_in_storage,
			)?;
			let bounded = T::Preimages::bound(proposal.clone())?.transmute();
			last_hash = T::Hashing::hash_of(&bounded);
		}

		let index = p - 1;
		// Have most everyone vote nay on last proposal, while keeping it from passing.
		for j in 0 .. m - 2 {
			let voter = &members[j as usize];
			let approve = false;
			Collective::<T, I>::vote(
				SystemOrigin::Signed(voter.clone()).into(),
				0,
				last_hash,
				index,
				approve,
			)?;
		}
		// Voter votes aye without resolving the vote.
		let approve = true;
		Collective::<T, I>::vote(
			SystemOrigin::Signed(voter.clone()).into(),
			0,
			last_hash,
			index,
			approve,
		)?;

		assert_eq!(Collective::<T, I>::proposals(0).len(), p as usize);

		// Whitelist voter account from further DB operations.
		let voter_key = frame_system::Account::<T>::hashed_key_for(&voter);
		frame_benchmarking::benchmarking::add_to_whitelist(voter_key.into());
	}: close(SystemOrigin::Signed(voter), 0, last_hash, index, Weight::max_value(), bytes_in_storage)
	verify {
		// The last proposal is removed.
		assert_eq!(Collective::<T, I>::proposals(0).len(), (p - 1) as usize);
		assert_second_to_last_event::<T, I>(
			Event::Disapproved {
				dao_id: 0,
				proposal_index: p - 1,
				proposal_hash: last_hash
			}.into()
		);
	}

	close_early_approved {
		let b in 1 .. MAX_BYTES;
		// We choose 4 as a minimum so we always trigger a vote in the voting loop (`for j in ...`)
		let m in 4 .. T::MaxMembers::get();
		let p in 1 .. T::MaxProposals::get();

		let bytes_in_storage = b + size_of::<u32>() as u32;

		create_dao::<T, I>()?;

		// Construct `members`.
		let mut members = vec![];
		for i in 0 .. m - 1 {
			let member = account::<T::AccountId>("member", i, SEED);
			members.push(member);
		}
		let caller: T::AccountId = whitelisted_caller();
		members.push(caller.clone());
		Collective::<T, I>::initialize_members(0, members.clone())?;

		// Add previous proposals
		let mut last_hash = T::Hash::default();
		for i in 0 .. p {
			// Proposals should be different so that different proposal hashes are generated
			let proposal: T::Proposal =
				SystemCall::<T>::remark { remark: vec![i as u8; b as usize] }.into();
			Collective::<T, I>::propose(
				SystemOrigin::Signed(caller.clone()).into(),
				0,
				Box::new(proposal.clone()),
				bytes_in_storage,
			)?;
			let bounded = T::Preimages::bound(proposal.clone())?.transmute();
			last_hash = T::Hashing::hash_of(&bounded);
		}

		// Caller switches vote to nay on their own proposal, allowing them to be the deciding approval vote
		Collective::<T, I>::vote(
			SystemOrigin::Signed(caller.clone()).into(),
			0,
			last_hash,
			p - 1,
			false,
		)?;

		// Have almost everyone vote nay on last proposal, while keeping it from failing.
		for j in 2 .. m - 1 {
			let voter = &members[j as usize];
			let approve = true;
			Collective::<T, I>::vote(
				SystemOrigin::Signed(voter.clone()).into(),
				0,
				last_hash,
				p - 1,
				approve,
			)?;
		}

		// Member zero is the first aye
		Collective::<T, I>::vote(
			SystemOrigin::Signed(members[0].clone()).into(),
			0,
			last_hash,
			p - 1,
			true,
		)?;

		assert_eq!(Collective::<T, I>::proposals(0).len(), p as usize);

		// Caller switches vote to aye, which passes the vote
		let index = p - 1;
		let approve = true;
		Collective::<T, I>::vote(
			SystemOrigin::Signed(caller.clone()).into(),
			0,
			last_hash,
			index, approve,
		)?;

	}: close(
		SystemOrigin::Signed(caller),
		0,
		last_hash,
		index,
		Weight::max_value(),
		bytes_in_storage
	)
	verify {
		// The last proposal is removed.
		assert_eq!(Collective::<T, I>::proposals(0).len(), (p - 1) as usize);
		assert_second_to_last_event::<T, I>(
			Event::Executed {
				dao_id: 0,
				proposal_index: p - 1,
				proposal_hash: last_hash,
				result: Err(DispatchError::BadOrigin)
			}.into()
		);
	}

	// TODO: get back to it in case if prime member returned back
	close_disapproved {
		// We choose 4 as a minimum so we always trigger a vote in the voting loop (`for j in ...`)
		let m in 4 .. T::MaxMembers::get();
		let p in 1 .. T::MaxProposals::get();

		let bytes = 100;
		let bytes_in_storage = bytes + size_of::<u32>() as u32;

		create_dao::<T, I>()?;

		// Construct `members`.
		let mut members = vec![];
		for i in 0 .. m - 1 {
			let member = account::<T::AccountId>("member", i, SEED);
			members.push(member);
		}
		let caller: T::AccountId = whitelisted_caller();
		members.push(caller.clone());
		Collective::<T, I>::initialize_members(
			0,
			members.clone()
		)?;

		// Add proposals
		let mut last_hash = T::Hash::default();
		for i in 0 .. p {
			// Proposals should be different so that different proposal hashes are generated
			let proposal: T::Proposal = SystemCall::<T>::remark {

				remark: vec![i as u8; bytes as usize]
			}.into();
			Collective::<T, I>::propose(
				SystemOrigin::Signed(caller.clone()).into(),
				0,
				Box::new(proposal.clone()),
				bytes_in_storage,
			)?;
			let bounded = T::Preimages::bound(proposal.clone())?.transmute();
			last_hash = T::Hashing::hash_of(&bounded);
		}

		let index = p - 1;
		// Have almost everyone vote aye on last proposal, while keeping it from passing.
		// A few abstainers will be the nay votes needed to fail the vote.
		for j in 2 .. m - 1 {
			let voter = &members[j as usize];
			let approve = false;
			Collective::<T, I>::vote(
				SystemOrigin::Signed(voter.clone()).into(),
				0,
				last_hash,
				index,
				approve,
			)?;
		}

		Collective::<T, I>::vote(
			SystemOrigin::Signed(caller.clone()).into(),
			0,
			last_hash,
			index,
			false,
		)?;

		System::<T>::set_block_number(T::BlockNumber::max_value());
		assert_eq!(Collective::<T, I>::proposals(0).len(), p as usize);
	}: close(
		SystemOrigin::Signed(caller),
		0,
		last_hash,
		index,
		Weight::max_value(), bytes_in_storage
	)
	verify {
		assert_eq!(Collective::<T, I>::proposals(0).len(), (p - 1) as usize);
		assert_second_to_last_event::<T, I>(
			Event::Disapproved {
				dao_id: 0,
				proposal_index: p - 1,
				proposal_hash: last_hash
			}.into()
		);
	}

	close_approved {
		let b in 1 .. MAX_BYTES;
		// We choose 4 as a minimum so we always trigger a vote in the voting loop (`for j in ...`)
		let m in 4 .. T::MaxMembers::get();
		let p in 1 .. T::MaxProposals::get();

		let bytes_in_storage = b + size_of::<u32>() as u32;

		create_dao::<T, I>()?;

		// Construct `members`.
		let mut members = vec![];
		for i in 0 .. m - 1 {
			let member = account::<T::AccountId>("member", i, SEED);
			members.push(member);
		}
		let caller: T::AccountId = whitelisted_caller();
		members.push(caller.clone());
		Collective::<T, I>::initialize_members(
			0,
			members.clone()
		)?;

		// Add proposals
		let mut last_hash = T::Hash::default();
		for i in 0 .. p {
			// Proposals should be different so that different proposal hashes are generated
			let proposal: T::Proposal =
				SystemCall::<T>::remark { remark: vec![i as u8; b as usize] }.into();
			Collective::<T, I>::propose(
				SystemOrigin::Signed(caller.clone()).into(),
				0,
				Box::new(proposal.clone()),
				bytes_in_storage,
			)?;
			let bounded = T::Preimages::bound(proposal.clone())?.transmute();
			last_hash = T::Hashing::hash_of(&bounded);
		}

		Collective::<T, _>::vote(
			SystemOrigin::Signed(caller.clone()).into(),
			0,
			last_hash,
			p - 1,
			true // Vote aye.
		)?;

		// Have almost everyone vote yay on last proposal, while keeping it from failing.
		// A few abstainers will be the aye votes needed to pass the vote.
		for j in 2 .. m - 1 {
			let voter = &members[j as usize];
			let approve = true;
			Collective::<T, I>::vote(
				SystemOrigin::Signed(voter.clone()).into(),
				0,
				last_hash,
				p - 1,
				approve
			)?;
		}

		System::<T>::set_block_number(T::BlockNumber::max_value());
		assert_eq!(Collective::<T, I>::proposals(0).len(), p as usize);

	}: close(
		SystemOrigin::Signed(caller),
		0,
		last_hash,
		p - 1,
		Weight::max_value(), bytes_in_storage
	)
	verify {
		assert_eq!(Collective::<T, I>::proposals(0).len(), (p - 1) as usize);
		assert_second_to_last_event::<T, I>(
			Event::Executed {
				dao_id: 0,
				proposal_index: p - 1,
				proposal_hash: last_hash,
				result: Err(DispatchError::BadOrigin)
			}.into()
		);
	}

	impl_benchmark_test_suite!(Collective, crate::tests::new_test_ext(), crate::tests::Test);
}
