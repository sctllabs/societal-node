//! bounties pallet benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use dao_primitives::{DaoOrigin, DaoPolicyProportion, SpendDaoFunds};
use frame_benchmarking::{account, benchmarks_instance_pallet, whitelisted_caller, BenchmarkError};
use frame_system::RawOrigin;
use serde_json::json;
use sp_runtime::traits::Bounded;

use crate::Pallet as Bounties;
use pallet_dao_treasury::Pallet as Treasury;

const SEED: u32 = 0;

// Create the pre-requisite information needed to create a dao.
fn setup_dao_payload<T: Config<I>, I: 'static>() -> Vec<u8> {
	/// <SBP MR2
	///
	/// I have a concern about this setup_dao_payload fn that seems to be common for all the
	/// benchmarks:
	///
	/// I understand this function is to initilize the payload for the dao during the benchmarking
	/// executions. As per i was able to understand, these values are mostly used by the methods
	/// exposed by T:DaoProvider which as far as i have reviewed, it does not execute any o(n)
	/// operations, but as the code extense and i don't have it 100% on top of my head i wonder if
	/// there is any part of the code where the function being benchmarked is influenced by the size
	/// of any of the fields in the json, which are currently hardcoded.
	///
	/// In case they are not, this comment can be dismissed.
	///
	/// >
	let dao_json = json!({
		"name": "name",
		"purpose": "purpose",
		"metadata": "metadata",
		"policy": {
			"proposal_period": 300000
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
	T::DaoProvider::create_dao(caller, vec![], vec![], data)?;

	let dao_account_id = T::DaoProvider::dao_account_id(0);
	let _ = T::Currency::make_free_balance_be(&dao_account_id, 1_000_000_000u32.into());

	Ok(())
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

// Create bounties that are approved for use in `on_initialize`.
fn create_approved_bounties<T: Config<I>, I: 'static>(n: u32) -> Result<(), &'static str> {
	create_dao::<T, I>()?;
	for i in 0..n {
		let (dao_id, _origin, _caller, _curator, _fee, value, reason) =
			setup_bounty::<T, I>(i, T::MaximumReasonLength::get());
		let approve_origin = get_dao_origin::<T, I>(0)?;
		Bounties::<T, I>::create_bounty(approve_origin, dao_id, value, reason)?;
	}
	ensure!(BountyApprovals::<T, I>::get(0).len() == n as usize, "Not all bounty approved");
	Ok(())
}

// Create the pre-requisite information needed to create a treasury `create_bounty`.
fn setup_bounty<T: Config<I>, I: 'static>(
	u: u32,
	d: u32,
) -> (
	u32,
	DaoOrigin<T::AccountId>,
	T::AccountId,
	T::AccountId,
	BalanceOf<T, I>,
	BalanceOf<T, I>,
	Vec<u8>,
) {
	let dao_id = 0_u32;
	let caller = account("caller", u, SEED);
	let value: BalanceOf<T, I> = 100u32.into();
	let fee = value / 2u32.into();
	let deposit = T::MaximumReasonLength::get().into();
	let _ = T::Currency::make_free_balance_be(&caller, deposit);
	let curator = account("curator", u, SEED);
	let _ = T::Currency::make_free_balance_be(&curator, fee);
	let reason = vec![0; d as usize];
	let origin = DaoOrigin {
		dao_account_id: account("member", 0, SEED),
		proportion: DaoPolicyProportion::AtLeast((1, 1)),
	};
	(dao_id, origin, caller, curator, fee, value, reason)
}

fn init_bounty<T: Config<I>, I: 'static>(
) -> Result<(u32, DaoOrigin<T::AccountId>, T::AccountId, BountyIndex), &'static str> {
	create_dao::<T, I>()?;
	let (dao_id, origin, _caller, curator, fee, value, reason) =
		setup_bounty::<T, I>(0, T::MaximumReasonLength::get());
	let approve_origin = get_dao_origin::<T, I>(0)?;
	Bounties::<T, I>::create_bounty(approve_origin.clone(), dao_id, value, reason)?;
	let bounty_id = BountyCount::<T, I>::get(dao_id) - 1;
	Treasury::<T, I>::spend_dao_funds(0);
	Bounties::<T, I>::propose_curator(approve_origin, dao_id, bounty_id, curator.clone(), fee)?;
	Bounties::<T, I>::accept_curator(RawOrigin::Signed(curator.clone()).into(), dao_id, bounty_id)?;
	Ok((dao_id, origin, curator, bounty_id))
}

fn assert_last_event<T: Config<I>, I: 'static>(generic_event: <T as Config<I>>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

/// <SBP MR2
///
/// Is there any reason why most of the benchmarks do not attach "verify"s functions?
/// I think its healty that every benchmark function verify that its execution was successful.
///
/// >
benchmarks_instance_pallet! {
	create_bounty {
		create_dao::<T, I>()?;
		let (dao_id, origin, caller, curator, fee, value, reason) = setup_bounty::<T, I>(0, T::MaximumReasonLength::get());
		let origin = get_dao_origin::<T, I>(0)?;
	}: _<T::RuntimeOrigin>(origin, dao_id, value, reason)

	propose_curator {
		create_dao::<T, I>()?;
		let (dao_id, origin, caller, curator, fee, value, reason) = setup_bounty::<T, I>(0, T::MaximumReasonLength::get());
		let approve_origin = get_dao_origin::<T, I>(0)?;
		Bounties::<T, I>::create_bounty(approve_origin.clone(), dao_id, value, reason)?;
		let bounty_id = BountyCount::<T, I>::get(dao_id) - 1;
		Treasury::<T, I>::spend_dao_funds(0);
	}: _<T::RuntimeOrigin>(approve_origin, dao_id, bounty_id, curator, fee)

	// Worst case when curator is inactive and any sender unassigns the curator.
	unassign_curator {
		let (dao_id, origin, curator, bounty_id) = init_bounty::<T, I>()?;
		let bounty_id = BountyCount::<T, I>::get(dao_id) - 1;
		let DaoPolicy { spend_period, .. } = T::DaoProvider::policy(0)?;
		frame_system::Pallet::<T>::set_block_number((spend_period.0 + 2u32).into());
		let caller: T::AccountId = whitelisted_caller();
	}: _(RawOrigin::Signed(caller), dao_id, bounty_id)

	accept_curator {
		create_dao::<T, I>()?;
		let (dao_id, origin, caller, curator, fee, value, reason) = setup_bounty::<T, I>(0, T::MaximumReasonLength::get());
		let approve_origin = get_dao_origin::<T, I>(0)?;
		Bounties::<T, I>::create_bounty(approve_origin.clone(), dao_id, value, reason)?;
		let bounty_id = BountyCount::<T, I>::get(dao_id) - 1;
		Treasury::<T, I>::spend_dao_funds(0);
		Bounties::<T, I>::propose_curator(approve_origin, dao_id, bounty_id, curator.clone(), fee)?;
	}: _(RawOrigin::Signed(curator), dao_id, bounty_id)

	award_bounty {
		let (dao_id, origin, curator, bounty_id) = init_bounty::<T, I>()?;
		Treasury::<T, I>::spend_dao_funds(0);
		let bounty_id = BountyCount::<T, I>::get(dao_id) - 1;
		let beneficiary = account("beneficiary", 0, SEED);
	}: _(RawOrigin::Signed(curator), dao_id, bounty_id, beneficiary)

	claim_bounty {
		let (dao_id, origin, curator, bounty_id) = init_bounty::<T, I>()?;
		Treasury::<T, I>::spend_dao_funds(0);
		let bounty_id = BountyCount::<T, I>::get(dao_id) - 1;
		let beneficiary_account: T::AccountId = account("beneficiary", 0, SEED);
		let beneficiary = beneficiary_account.clone();
		Bounties::<T, I>::award_bounty(RawOrigin::Signed(curator.clone()).into(), dao_id, bounty_id, beneficiary)?;
		let DaoPolicy { spend_period, .. } = T::DaoProvider::policy(0)?;
		frame_system::Pallet::<T>::set_block_number((spend_period.0 + 1u32).into());
		ensure!(T::Currency::free_balance(&beneficiary_account).is_zero(), "Beneficiary already has balance");
	}: _(RawOrigin::Signed(curator), dao_id, bounty_id)
	verify {
		ensure!(!T::Currency::free_balance(&beneficiary_account).is_zero(), "Beneficiary didn't get paid");
	}

	close_bounty_active {
		let (dao_id, origin, curator, bounty_id) = init_bounty::<T, I>()?;
		Treasury::<T, I>::spend_dao_funds(0);
		let bounty_id = BountyCount::<T, I>::get(dao_id) - 1;
		let approve_origin = get_dao_origin::<T, I>(0)?;
	}: close_bounty<T::RuntimeOrigin>(approve_origin, dao_id, bounty_id)
	verify {
		assert_last_event::<T, I>(Event::BountyCanceled { dao_id, index: bounty_id }.into())
	}

	extend_bounty_expiry {
		let (dao_id, origin, curator, bounty_id) = init_bounty::<T, I>()?;
		Treasury::<T, I>::spend_dao_funds(0);
		let bounty_id = BountyCount::<T, I>::get(dao_id) - 1;
		let DaoPolicy { spend_period, .. } = T::DaoProvider::policy(0)?;
		let current_block_number = frame_system::Pallet::<T>::block_number();
	}: _(RawOrigin::Signed(curator), dao_id, bounty_id, Vec::new())
	verify {}

	spend_funds {
		let b in 0 .. 100;

		create_approved_bounties::<T, I>(b)?;

		let dao_id = 0;
		let mut budget_remaining = BalanceOf::<T, I>::max_value();
		let mut imbalance = PositiveImbalanceOf::<T, I>::zero();
		let mut total_weight = Weight::zero();
		let mut missed_any = false;
	}: {
		<Bounties<T, I> as pallet_dao_treasury::SpendFunds<T, I>>::spend_funds(
			dao_id,
			&mut budget_remaining,
			&mut imbalance,
			&mut total_weight,
			&mut missed_any,
		);
	}
	verify {
		ensure!(missed_any == false, "Missed some");
		if b > 0 {
			ensure!(budget_remaining < BalanceOf::<T, I>::max_value(), "Budget not used");
			assert_last_event::<T, I>(Event::BountyBecameActive { dao_id, index: b - 1, status: BountyStatus::Funded }.into())
		} else {
			ensure!(budget_remaining == BalanceOf::<T, I>::max_value(), "Budget used");
		}
	}

	impl_benchmark_test_suite!(Bounties, crate::tests::new_test_ext(), crate::tests::Test)
}
