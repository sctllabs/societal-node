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

// Changes made comparing to the RuntimeOriginal tests of the FRAME pallet-collective:
// - added DAO pallet configuration as DAO provider
// - using DAO as a parameter for pallet functions

//! DAO Democracy pallet benchmarking.

use super::*;

use dao_primitives::{DaoOrigin, DaoPolicyProportion};
use frame_benchmarking::{account, benchmarks, whitelist_account, BenchmarkError};
use frame_support::{
	assert_noop, assert_ok,
	traits::{fungibles::Transfer, Currency, EnsureOrigin, EnsureOriginWithArg, Get},
};
use frame_system::RawOrigin;
use serde_json::json;
use sp_core::H256;
use sp_runtime::{traits::Bounded, BoundedVec};

use crate::Pallet as Democracy;

const SEED: u32 = 0;

// Create the pre-requisite information needed to create a dao.
fn setup_dao_payload<T: Config>() -> Vec<u8> {
	let dao_json = json!({
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

fn create_dao<T: Config>() -> Result<T::AccountId, DispatchError> {
	let caller: T::AccountId = account("caller", 0, SEED);
	let data = setup_dao_payload::<T>();

	T::DaoProvider::create_dao(caller.clone(), vec![caller.clone()], vec![caller.clone()], data)?;

	let dao_account_id = T::DaoProvider::dao_account_id(0);
	let _ = T::Currency::make_free_balance_be(&dao_account_id, 1_000_000_000u32.into());

	Ok(caller)
}

fn funded_account<T: Config>(
	name: &'static str,
	index: u32,
) -> Result<T::AccountId, DispatchError> {
	let caller: T::AccountId = account(name, index, SEED);
	// Give the account half of the maximum value of the `Balance` type.
	// Otherwise some transfers will fail with an overflow error.
	T::Currency::make_free_balance_be(&caller, BalanceOf::<T>::max_value() / 2u32.into());

	// Transferring some DAO tokens to the caller
	let dao_account_id = T::DaoProvider::dao_account_id(0);
	T::Assets::transfer(0, &dao_account_id, &caller, 10_000_u32.into(), true)?;

	Ok(caller)
}

fn make_proposal<T: Config>(n: u32) -> BoundedCallOf<T> {
	let call: CallOf<T> = frame_system::Call::remark { remark: n.encode() }.into();
	<T as Config>::Preimages::bound(call).unwrap()
}

fn add_proposal<T: Config>(n: u32) -> Result<H256, &'static str> {
	let other = funded_account::<T>("proposer", n)?;
	let value = 10_u32.into();
	let proposal = make_proposal::<T>(n);
	Democracy::<T>::propose(RawOrigin::Signed(other).into(), 0, proposal.clone(), value)?;
	Ok(proposal.hash())
}

fn add_referendum<T: Config>(n: u32) -> (ReferendumIndex, H256) {
	let vote_threshold = VoteThreshold::SimpleMajority;
	let proposal = make_proposal::<T>(n);
	let hash = proposal.hash();
	(
		Democracy::<T>::inject_referendum(
			0,
			T::BlockNumber::min_value(),
			0u32,
			proposal,
			vote_threshold,
			0u32.into(),
		),
		hash,
	)
}

fn account_vote<T: Config>(b: BalanceOf<T>) -> AccountVote<BalanceOf<T>> {
	let v = Vote { aye: true, conviction: Conviction::Locked1x };

	AccountVote::Standard { vote: v, balance: b }
}

benchmarks! {
	propose_with_meta {
		let p = T::MaxProposals::get();

		create_dao::<T>()?;

		for i in 0 .. (p - 1) {
			add_proposal::<T>(i)?;
		}

		let caller = funded_account::<T>("caller", 0)?;

		let proposal = make_proposal::<T>(0);
		let value = 10_u32.into();
		whitelist_account!(caller);

		let meta: Option<Vec<u8>> = Some(
			"Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor \
			incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud \
			exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure \
			dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. \
			Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt \
			mollit anim id est laborum. Lorem ipsum dolor sit amet, consectetur adipiscing elit, \
			sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim \
			veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo \
			consequat. Duis aute irure dolor in reprehenderit".into()
		);
	}: _(RawOrigin::Signed(caller), 0, proposal, value, meta)
	verify {
		assert_eq!(Democracy::<T>::public_props(0).len(), p as usize, "Proposals not created.");
	}

	second {
		create_dao::<T>()?;
		let caller = funded_account::<T>("caller", 0)?;
		add_proposal::<T>(0)?;

		// Create s existing "seconds"
		// we must reserve one deposit for the `proposal` and one for our benchmarked `second` call.
		for i in 0 .. T::MaxDeposits::get() - 2 {
			let seconder = funded_account::<T>("seconder", i)?;
			Democracy::<T>::second(RawOrigin::Signed(seconder).into(), 0, 0)?;
		}

		let deposits = Democracy::<T>::deposit_of(0, 0).ok_or("Proposal not created")?;
		assert_eq!(deposits.0.len(), (T::MaxDeposits::get() - 1) as usize, "Seconds not recorded");
		whitelist_account!(caller);
	}: _(RawOrigin::Signed(caller), 0, 0)
	verify {
		let deposits = Democracy::<T>::deposit_of(0, 0).ok_or("Proposal not created")?;
		assert_eq!(deposits.0.len(), (T::MaxDeposits::get()) as usize, "`second` benchmark did not work");
	}

	vote_new {
		create_dao::<T>()?;
		let caller = funded_account::<T>("caller", 0)?;
		let account_vote = account_vote::<T>(100u32.into());

		// We need to create existing direct votes
		for i in 0 .. T::MaxVotes::get() - 1 {
			let ref_index = add_referendum::<T>(i).0;
			Democracy::<T>::vote(
				RawOrigin::Signed(caller.clone()).into(), 0, ref_index, account_vote)?;
		}
		let votes = match VotingOf::<T>::get(0, &caller) {
			Voting::Direct { votes, .. } => votes,
			_ => return Err("Votes are not direct".into()),
		};
		assert_eq!(votes.len(), (T::MaxVotes::get() - 1) as usize, "Votes were not recorded.");

		let ref_index = add_referendum::<T>(T::MaxVotes::get() - 1).0;
		whitelist_account!(caller);
	}: vote(RawOrigin::Signed(caller.clone()), 0, ref_index, account_vote)
	verify {
		let votes = match VotingOf::<T>::get(0, &caller) {
			Voting::Direct { votes, .. } => votes,
			_ => return Err("Votes are not direct".into()),
		};
		assert_eq!(votes.len(), T::MaxVotes::get() as usize, "Vote was not recorded.");
	}

	vote_existing {
		create_dao::<T>()?;
		let caller = funded_account::<T>("caller", 0)?;
		let account_vote = account_vote::<T>(100u32.into());

		// We need to create existing direct votes
		for i in 0..T::MaxVotes::get() {
			let ref_index = add_referendum::<T>(i).0;
			Democracy::<T>::vote(
				RawOrigin::Signed(caller.clone()).into(), 0, ref_index, account_vote)?;
		}
		let votes = match VotingOf::<T>::get(0, &caller) {
			Voting::Direct { votes, .. } => votes,
			_ => return Err("Votes are not direct".into()),
		};
		assert_eq!(votes.len(), T::MaxVotes::get() as usize, "Votes were not recorded.");

		// Change vote from aye to nay
		let nay = Vote { aye: false, conviction: Conviction::Locked1x };
		let new_vote = AccountVote::Standard { vote: nay, balance: 1000u32.into() };
		let ref_index = Democracy::<T>::referendum_count(0) - 1;

		// This tests when a user changes a vote
		whitelist_account!(caller);
	}: vote(RawOrigin::Signed(caller.clone()), 0, ref_index, new_vote)
	verify {
		let votes = match VotingOf::<T>::get(0, &caller) {
			Voting::Direct { votes, .. } => votes,
			_ => return Err("Votes are not direct".into()),
		};
		assert_eq!(votes.len(), T::MaxVotes::get() as usize, "Vote was incorrectly added");
		let referendum_info = Democracy::<T>::referendum_info(0, ref_index)
			.ok_or("referendum doesn't exist")?;
		let tally =  match referendum_info {
			ReferendumInfo::Ongoing(r) => r.tally,
			_ => return Err("referendum not ongoing".into()),
		};
		assert_eq!(tally.nays, 1000u32.into(), "changed vote was not recorded");
	}

	emergency_cancel {
		let caller = create_dao::<T>()?;
		let dao_account_id: T::AccountId = T::DaoProvider::dao_account_id(0);
		let origin = T::CancellationOrigin::try_successful_origin(
			&DaoOrigin {
				dao_account_id,
				proportion: DaoPolicyProportion::AtLeast((0, 1))
			}).map_err(|_| BenchmarkError::Weightless)?;
		let ref_index = add_referendum::<T>(0).0;
		assert_ok!(Democracy::<T>::referendum_status(0, ref_index));
	}: _<T::RuntimeOrigin>(origin, 0, ref_index)
	verify {
		// Referendum has been canceled
		assert_noop!(
			Democracy::<T>::referendum_status(0, ref_index),
			Error::<T>::ReferendumInvalid,
		);
	}

	blacklist {
		create_dao::<T>()?;

		// Place our proposal at the end to make sure it's worst case.
		for i in 0 .. T::MaxProposals::get() - 1 {
			add_proposal::<T>(i)?;
		}
		// We should really add a lot of seconds here, but we're not doing it elsewhere.

		// Add a referendum of our proposal.
		let (ref_index, hash) = add_referendum::<T>(0);
		assert_ok!(Democracy::<T>::referendum_status(0, ref_index));

		let dao_account_id: T::AccountId = T::DaoProvider::dao_account_id(0);

		// Place our proposal in the external queue, too.
		assert_ok!(
			Democracy::<T>::external_propose(
				T::ExternalOrigin::try_successful_origin(
					&DaoOrigin {
						dao_account_id: dao_account_id.clone(),
						proportion: DaoPolicyProportion::AtLeast((0, 1))
					}
				)
				.map_err(|_| BenchmarkError::Weightless)?, 0, make_proposal::<T>(0))
		);
		let origin = T::BlacklistOrigin::try_successful_origin(
			&DaoOrigin {
				dao_account_id,
				proportion: DaoPolicyProportion::AtLeast((0, 1))
			}
		).map_err(|_| BenchmarkError::Weightless)?;
	}: _<T::RuntimeOrigin>(origin, 0, hash, Some(ref_index))
	verify {
		// Referendum has been canceled
		assert_noop!(
			Democracy::<T>::referendum_status(0, ref_index),
			Error::<T>::ReferendumInvalid
		);
	}

	// Worst case scenario, we external propose a previously blacklisted proposal
	external_propose {
		create_dao::<T>()?;
		let dao_account_id: T::AccountId = T::DaoProvider::dao_account_id(0);
		let origin = T::ExternalOrigin::try_successful_origin(
			&DaoOrigin {
				dao_account_id,
				proportion: DaoPolicyProportion::AtLeast((0, 1))
			}
		).map_err(|_| BenchmarkError::Weightless)?;
		let proposal = make_proposal::<T>(0);
		// Add proposal to blacklist with block number 0

		let addresses: BoundedVec<_, _> = (0..(T::MaxBlacklisted::get() - 1))
			.into_iter()
			.map(|i| account::<T::AccountId>("blacklist", i, SEED))
			.collect::<Vec<_>>()
			.try_into()
			.unwrap();
		Blacklist::<T>::insert(0, proposal.hash(), (T::BlockNumber::zero(), addresses));
	}: _<T::RuntimeOrigin>(origin, 0, proposal)
	verify {
		// External proposal created
		ensure!(<NextExternal<T>>::contains_key(0), "External proposal didn't work");
	}

	external_propose_majority {
		create_dao::<T>()?;
		let dao_account_id: T::AccountId = T::DaoProvider::dao_account_id(0);
		let dao_origin = DaoOrigin { dao_account_id, proportion: DaoPolicyProportion::AtLeast((0, 1)) };
		let origin = T::ExternalMajorityOrigin::try_successful_origin(&dao_origin)
			.map_err(|_| BenchmarkError::Weightless)?;
		let proposal = make_proposal::<T>(0);
	}: _<T::RuntimeOrigin>(origin, 0, proposal)
	verify {
		// External proposal created
		ensure!(<NextExternal<T>>::contains_key(0), "External proposal didn't work");
	}

	external_propose_default {
		create_dao::<T>()?;
		let dao_account_id: T::AccountId = T::DaoProvider::dao_account_id(0);
		let dao_origin = DaoOrigin { dao_account_id, proportion: DaoPolicyProportion::AtLeast((0, 1)) };
		let origin = T::ExternalDefaultOrigin::try_successful_origin(&dao_origin)
			.map_err(|_| BenchmarkError::Weightless)?;
		let proposal = make_proposal::<T>(0);
	}: _<T::RuntimeOrigin>(origin, 0, proposal)
	verify {
		// External proposal created
		ensure!(<NextExternal<T>>::contains_key(0), "External proposal didn't work");
	}

	fast_track {
		create_dao::<T>()?;
		let dao_account_id: T::AccountId = T::DaoProvider::dao_account_id(0);
		let dao_origin = DaoOrigin { dao_account_id, proportion: DaoPolicyProportion::AtLeast((0, 1)) };
		let origin_propose = T::ExternalDefaultOrigin::try_successful_origin(&dao_origin)
			.map_err(|_| BenchmarkError::Weightless)?;
		let proposal = make_proposal::<T>(0);
		let proposal_hash = proposal.hash();
		Democracy::<T>::external_propose_default(origin_propose, 0, proposal)?;

		// NOTE: Instant origin may invoke a little bit more logic, but may not always succeed.
		let origin_fast_track = T::FastTrackOrigin::try_successful_origin(&dao_origin)
			.map_err(|_| BenchmarkError::Weightless)?;
		let voting_period = T::BlockNumber::min_value() + T::BlockNumber::try_from(1_u64).ok().unwrap();
		let delay = 0u32;
	}: _<T::RuntimeOrigin>(origin_fast_track, 0, proposal_hash, voting_period, delay.into())
	verify {
		assert_eq!(Democracy::<T>::referendum_count(0), 1, "referendum not created")
	}

	veto_external {
		let caller = create_dao::<T>()?;
		let dao_account_id: T::AccountId = T::DaoProvider::dao_account_id(0);
		let dao_origin = DaoOrigin { dao_account_id, proportion: DaoPolicyProportion::AtLeast((0, 1)) };

		let proposal = make_proposal::<T>(0);
		let proposal_hash = proposal.hash();

		let origin_propose = T::ExternalDefaultOrigin::try_successful_origin(&dao_origin)
			.map_err(|_| BenchmarkError::Weightless)?;
		Democracy::<T>::external_propose_default(origin_propose, 0, proposal)?;

		let mut vetoers: BoundedVec<T::AccountId, _> = Default::default();
		for i in 0 .. (T::MaxBlacklisted::get() - 1) {
			vetoers.try_push(account::<T::AccountId>("vetoer", i, SEED)).unwrap();
		}
		vetoers.sort();
		Blacklist::<T>::insert(0, proposal_hash, (T::BlockNumber::zero(), vetoers));

		let origin = T::VetoOrigin::try_successful_origin().map_err(|_| BenchmarkError::Weightless)?;
		ensure!(NextExternal::<T>::get(0).is_some(), "no external proposal");
	}: _<T::RuntimeOrigin>(origin, 0, proposal_hash)
	verify {
		assert!(NextExternal::<T>::get(0).is_none());
		let (_, new_vetoers) = <Blacklist<T>>::get(0, &proposal_hash).ok_or("no blacklist")?;
		assert_eq!(new_vetoers.len(), T::MaxBlacklisted::get() as usize, "vetoers not added");
	}

	cancel_proposal {
		create_dao::<T>()?;
		let dao_account_id: T::AccountId = T::DaoProvider::dao_account_id(0);
		let dao_origin = DaoOrigin { dao_account_id, proportion: DaoPolicyProportion::AtLeast((0, 1)) };

		// Place our proposal at the end to make sure it's worst case.
		for i in 0 .. T::MaxProposals::get() {
			add_proposal::<T>(i)?;
		}
		let cancel_origin = T::CancelProposalOrigin::try_successful_origin(&dao_origin)
			.map_err(|_| BenchmarkError::Weightless)?;
	}: _<T::RuntimeOrigin>(cancel_origin, 0, 0)

	cancel_referendum {
		create_dao::<T>()?;
		let dao_account_id: T::AccountId = T::DaoProvider::dao_account_id(0);
		let dao_origin = DaoOrigin { dao_account_id, proportion: DaoPolicyProportion::AtLeast((0, 1)) };

		let ref_index = add_referendum::<T>(0).0;

		let cancel_origin = T::CancelProposalOrigin::try_successful_origin(&dao_origin)
			.map_err(|_| BenchmarkError::Weightless)?;
	}: _<T::RuntimeOrigin>(cancel_origin, 0, ref_index)

	delegate {
		let r in 0 .. (T::MaxVotes::get() - 1);

		create_dao::<T>()?;

		let initial_balance: BalanceOf<T> = 100u32.into();
		let delegated_balance: BalanceOf<T> = 1000u32.into();

		let caller = funded_account::<T>("caller", 0)?;
		// Caller will initially delegate to `old_delegate`
		let old_delegate: T::AccountId = funded_account::<T>("old_delegate", r)?;
		let old_delegate_lookup = T::Lookup::unlookup(old_delegate.clone());
		Democracy::<T>::delegate(
			RawOrigin::Signed(caller.clone()).into(),
			0,
			old_delegate_lookup,
			Conviction::Locked1x,
			delegated_balance,
		)?;
		let (target, balance) = match VotingOf::<T>::get(0, &caller) {
			Voting::Delegating { target, balance, .. } => (target, balance),
			_ => return Err("Votes are not direct".into()),
		};
		assert_eq!(target, old_delegate, "delegation target didn't work");
		assert_eq!(balance, delegated_balance, "delegation balance didn't work");
		// Caller will now switch to `new_delegate`
		let new_delegate: T::AccountId = funded_account::<T>("new_delegate", r)?;
		let new_delegate_lookup = T::Lookup::unlookup(new_delegate.clone());
		let account_vote = account_vote::<T>(initial_balance);
		// We need to create existing direct votes for the `new_delegate`
		for i in 0..r {
			let ref_index = add_referendum::<T>(i).0;
			Democracy::<T>::vote(
				RawOrigin::Signed(new_delegate.clone()).into(), 0, ref_index, account_vote)?;
		}
		let votes = match VotingOf::<T>::get(0, &new_delegate) {
			Voting::Direct { votes, .. } => votes,
			_ => return Err("Votes are not direct".into()),
		};
		assert_eq!(votes.len(), r as usize, "Votes were not recorded.");
		whitelist_account!(caller);
	}: _(RawOrigin::Signed(caller.clone()), 0, new_delegate_lookup, Conviction::Locked1x, delegated_balance)
	verify {
		let (target, balance) = match VotingOf::<T>::get(0, &caller) {
			Voting::Delegating { target, balance, .. } => (target, balance),
			_ => return Err("Votes are not direct".into()),
		};
		assert_eq!(target, new_delegate, "delegation target didn't work");
		assert_eq!(balance, delegated_balance, "delegation balance didn't work");
		let delegations = match VotingOf::<T>::get(0, &new_delegate) {
			Voting::Direct { delegations, .. } => delegations,
			_ => return Err("Votes are not direct".into()),
		};
		assert_eq!(delegations.capital, delegated_balance, "delegation was not recorded.");
	}

	undelegate {
		let r in 0 .. (T::MaxVotes::get() - 1);

		create_dao::<T>()?;

		let initial_balance: BalanceOf<T> = 100u32.into();
		let delegated_balance: BalanceOf<T> = 1000u32.into();

		let caller = funded_account::<T>("caller", 0)?;
		// Caller will delegate
		let the_delegate: T::AccountId = funded_account::<T>("delegate", r)?;
		let the_delegate_lookup = T::Lookup::unlookup(the_delegate.clone());
		Democracy::<T>::delegate(
			RawOrigin::Signed(caller.clone()).into(),
			0,
			the_delegate_lookup,
			Conviction::Locked1x,
			delegated_balance,
		)?;
		let (target, balance) = match VotingOf::<T>::get(0, &caller) {
			Voting::Delegating { target, balance, .. } => (target, balance),
			_ => return Err("Votes are not direct".into()),
		};
		assert_eq!(target, the_delegate, "delegation target didn't work");
		assert_eq!(balance, delegated_balance, "delegation balance didn't work");
		// We need to create votes direct votes for the `delegate`
		let account_vote = account_vote::<T>(initial_balance);
		for i in 0..r {
			let ref_index = add_referendum::<T>(i).0;
			Democracy::<T>::vote(
				RawOrigin::Signed(the_delegate.clone()).into(),
				0,
				ref_index,
				account_vote
			)?;
		}
		let votes = match VotingOf::<T>::get(0, &the_delegate) {
			Voting::Direct { votes, .. } => votes,
			_ => return Err("Votes are not direct".into()),
		};
		assert_eq!(votes.len(), r as usize, "Votes were not recorded.");
		whitelist_account!(caller);
	}: _(RawOrigin::Signed(caller.clone()), 0)
	verify {
		// Voting should now be direct
		match VotingOf::<T>::get(0, &caller) {
			Voting::Direct { .. } => (),
			_ => return Err("undelegation failed".into()),
		}
	}

	// Test when unlock will remove locks
	unlock_remove {
		let r in 0 .. (T::MaxVotes::get() - 1);

		create_dao::<T>()?;

		let locker = funded_account::<T>("locker", 0)?;
		let locker_lookup = T::Lookup::unlookup(locker.clone());
		// Populate votes so things are locked
		let base_balance: BalanceOf<T> = 100u32.into();
		let small_vote = account_vote::<T>(base_balance);
		// Vote and immediately unvote
		for i in 0 .. r {
			let ref_index = add_referendum::<T>(i).0;
			Democracy::<T>::vote(RawOrigin::Signed(locker.clone()).into(), 0, ref_index, small_vote)?;
			Democracy::<T>::remove_vote(RawOrigin::Signed(locker.clone()).into(), 0, ref_index)?;
		}

		let caller = funded_account::<T>("caller", 0)?;
		whitelist_account!(caller);
	}: unlock(RawOrigin::Signed(caller), 0, locker_lookup)
	verify {
		// Note that we may want to add a `get_lock` api to actually verify
		let voting = VotingOf::<T>::get(0, &locker);
		assert_eq!(voting.locked_balance(), BalanceOf::<T>::zero());
	}

	// Test when unlock will set a new value
	unlock_set {
		let r in 0 .. (T::MaxVotes::get() - 1);

		create_dao::<T>()?;

		let locker = funded_account::<T>("locker", 0)?;
		let locker_lookup = T::Lookup::unlookup(locker.clone());
		// Populate votes so things are locked
		let base_balance: BalanceOf<T> = 100u32.into();
		let small_vote = account_vote::<T>(base_balance);
		for i in 0 .. r {
			let ref_index = add_referendum::<T>(i).0;
			Democracy::<T>::vote(RawOrigin::Signed(locker.clone()).into(), 0, ref_index, small_vote)?;
		}

		// Create a big vote so lock increases
		let big_vote = account_vote::<T>(base_balance * 10u32.into());
		let ref_index = add_referendum::<T>(r).0;
		Democracy::<T>::vote(RawOrigin::Signed(locker.clone()).into(), 0, ref_index, big_vote)?;

		let votes = match VotingOf::<T>::get(0, &locker) {
			Voting::Direct { votes, .. } => votes,
			_ => return Err("Votes are not direct".into()),
		};
		assert_eq!(votes.len(), (r + 1) as usize, "Votes were not recorded.");

		let voting = VotingOf::<T>::get(0, &locker);
		assert_eq!(voting.locked_balance(), base_balance * 10u32.into());

		Democracy::<T>::remove_vote(RawOrigin::Signed(locker.clone()).into(), 0, ref_index)?;

		let caller = funded_account::<T>("caller", 0)?;
		whitelist_account!(caller);
	}: unlock(RawOrigin::Signed(caller), 0, locker_lookup)
	verify {
		let votes = match VotingOf::<T>::get(0, &locker) {
			Voting::Direct { votes, .. } => votes,
			_ => return Err("Votes are not direct".into()),
		};
		assert_eq!(votes.len(), r as usize, "Vote was not removed");

		let voting = VotingOf::<T>::get(0, &locker);
		// Note that we may want to add a `get_lock` api to actually verify
		assert_eq!(voting.locked_balance(), if r > 0 { base_balance } else { 0u32.into() });
	}

	remove_vote {
		let r in 1 .. T::MaxVotes::get();

		create_dao::<T>()?;

		let caller = funded_account::<T>("caller", 0)?;
		let account_vote = account_vote::<T>(100u32.into());

		for i in 0 .. r {
			let ref_index = add_referendum::<T>(i).0;
			Democracy::<T>::vote(
				RawOrigin::Signed(caller.clone()).into(), 0, ref_index, account_vote)?;
		}

		let votes = match VotingOf::<T>::get(0, &caller) {
			Voting::Direct { votes, .. } => votes,
			_ => return Err("Votes are not direct".into()),
		};
		assert_eq!(votes.len(), r as usize, "Votes not created");

		let ref_index = r - 1;
		whitelist_account!(caller);
	}: _(RawOrigin::Signed(caller.clone()), 0, ref_index)
	verify {
		let votes = match VotingOf::<T>::get(0, &caller) {
			Voting::Direct { votes, .. } => votes,
			_ => return Err("Votes are not direct".into()),
		};
		assert_eq!(votes.len(), (r - 1) as usize, "Vote was not removed");
	}

	// Worst case is when target == caller and referendum is ongoing
	remove_other_vote {
		let r in 1 .. T::MaxVotes::get();

		create_dao::<T>()?;

		let caller = funded_account::<T>("caller", r)?;
		let caller_lookup = T::Lookup::unlookup(caller.clone());
		let account_vote = account_vote::<T>(100u32.into());

		for i in 0 .. r {
			let ref_index = add_referendum::<T>(i).0;
			Democracy::<T>::vote(
				RawOrigin::Signed(caller.clone()).into(), 0, ref_index, account_vote)?;
		}

		let votes = match VotingOf::<T>::get(0, &caller) {
			Voting::Direct { votes, .. } => votes,
			_ => return Err("Votes are not direct".into()),
		};
		assert_eq!(votes.len(), r as usize, "Votes not created");

		let ref_index = r - 1;
		whitelist_account!(caller);
	}: _(RawOrigin::Signed(caller.clone()), 0, caller_lookup, ref_index)
	verify {
		let votes = match VotingOf::<T>::get(0, &caller) {
			Voting::Direct { votes, .. } => votes,
			_ => return Err("Votes are not direct".into()),
		};
		assert_eq!(votes.len(), (r - 1) as usize, "Vote was not removed");
	}

	impl_benchmark_test_suite!(
		Democracy,
		crate::tests::new_test_ext(),
		crate::tests::Test
	);
}
