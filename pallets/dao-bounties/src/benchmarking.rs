//! bounties pallet benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use dao_primitives::{DaoOrigin, DaoPolicyProportion};
use frame_benchmarking::{account, benchmarks_instance_pallet, whitelisted_caller, BenchmarkError};
use frame_support::traits::EnsureOriginWithArg;
use frame_system::RawOrigin;
use sp_runtime::traits::Bounded;

use crate::Pallet as Bounties;
use pallet_dao_treasury::Pallet as Treasury;

const SEED: u32 = 0;

// Create bounties that are approved for use in `on_initialize`.
fn create_approved_bounties<T: Config<I>, I: 'static>(n: u32) -> Result<(), &'static str> {
	for i in 0..n {
		let (dao_id, origin, _caller, _curator, _fee, value, reason) =
			setup_bounty::<T, I>(i, T::MaximumReasonLength::get());
		let approve_origin = T::ApproveOrigin::try_successful_origin(&origin)
			.map_err(|_| BenchmarkError::Weightless)?;
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
	let _ = T::Currency::make_free_balance_be(&curator, fee / 2u32.into());
	let reason = vec![0; d as usize];
	let origin = DaoOrigin {
		dao_account_id: account("member", 0, SEED),
		proportion: DaoPolicyProportion::AtLeast((1, 1)),
	};
	(dao_id, origin, caller, curator, fee, value, reason)
}

fn init_bounty<T: Config<I>, I: 'static>(
) -> Result<(u32, DaoOrigin<T::AccountId>, T::AccountId, BountyIndex), &'static str> {
	let (dao_id, origin, _caller, curator, fee, value, reason) =
		setup_bounty::<T, I>(0, T::MaximumReasonLength::get());
	let approve_origin =
		T::ApproveOrigin::try_successful_origin(&origin).map_err(|_| BenchmarkError::Weightless)?;
	Bounties::<T, I>::create_bounty(approve_origin.clone(), dao_id, value, reason)?;
	let bounty_id = BountyCount::<T, I>::get(dao_id) - 1;
	Treasury::<T, I>::on_initialize(T::BlockNumber::zero());
	Bounties::<T, I>::propose_curator(approve_origin, dao_id, bounty_id, curator.clone(), fee)?;
	Bounties::<T, I>::accept_curator(RawOrigin::Signed(curator.clone()).into(), dao_id, bounty_id)?;
	Ok((dao_id, origin, curator, bounty_id))
}

fn setup_pot_account<T: Config<I>, I: 'static>() {
	// let pot_account = Bounties::<T, I>::account_id();
	// let value = T::Currency::minimum_balance().saturating_mul(1_000_000_000u32.into());
	// let _ = T::Currency::make_free_balance_be(&pot_account, value);
}

fn assert_last_event<T: Config<I>, I: 'static>(generic_event: <T as Config<I>>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

benchmarks_instance_pallet! {
	create_bounty {
		let (dao_id, origin, caller, curator, fee, value, reason) = setup_bounty::<T, I>(0, T::MaximumReasonLength::get());
		let approve_origin = T::ApproveOrigin::try_successful_origin(&origin).map_err(|_| BenchmarkError::Weightless)?;
	}: _<T::RuntimeOrigin>(approve_origin, dao_id, value, reason)

	propose_curator {
		setup_pot_account::<T, I>();
		let (dao_id, origin, caller, curator, fee, value, reason) = setup_bounty::<T, I>(0, T::MaximumReasonLength::get());
		let approve_origin = T::ApproveOrigin::try_successful_origin(&origin).map_err(|_| BenchmarkError::Weightless)?;
		Bounties::<T, I>::create_bounty(approve_origin, dao_id, value, reason)?;
		let bounty_id = BountyCount::<T, I>::get(dao_id) - 1;
		Treasury::<T, I>::on_initialize(T::BlockNumber::zero());
		let approve_origin = T::ApproveOrigin::try_successful_origin(&origin).map_err(|_| BenchmarkError::Weightless)?;
	}: _<T::RuntimeOrigin>(approve_origin, dao_id, bounty_id, curator, fee)

	// Worst case when curator is inactive and any sender unassigns the curator.
	unassign_curator {
		setup_pot_account::<T, I>();
		let (dao_id, origin, curator, bounty_id) = init_bounty::<T, I>()?;
		Treasury::<T, I>::on_initialize(T::BlockNumber::zero());
		let bounty_id = BountyCount::<T, I>::get(dao_id) - 1;
		// frame_system::Pallet::<T>::set_block_number(T::BountyUpdatePeriod::get() + 2u32.into());
		frame_system::Pallet::<T>::set_block_number(2u32.into());
		let caller = whitelisted_caller();
	}: _(RawOrigin::Signed(caller), dao_id, bounty_id)

	accept_curator {
		setup_pot_account::<T, I>();
		let (dao_id, origin, caller, curator, fee, value, reason) = setup_bounty::<T, I>(0, T::MaximumReasonLength::get());
		let approve_origin = T::ApproveOrigin::try_successful_origin(&origin).map_err(|_| BenchmarkError::Weightless)?;
		Bounties::<T, I>::create_bounty(approve_origin.clone(), dao_id, value, reason)?;
		let bounty_id = BountyCount::<T, I>::get(dao_id) - 1;
		Treasury::<T, I>::on_initialize(T::BlockNumber::zero());
		Bounties::<T, I>::propose_curator(approve_origin, dao_id, bounty_id, curator.clone(), fee)?;
	}: _(RawOrigin::Signed(curator), dao_id, bounty_id)

	award_bounty {
		setup_pot_account::<T, I>();
		let (dao_id, origin, curator, bounty_id) = init_bounty::<T, I>()?;
		Treasury::<T, I>::on_initialize(T::BlockNumber::zero());

		let bounty_id = BountyCount::<T, I>::get(dao_id) - 1;

		let beneficiary = account("beneficiary", 0, SEED);
	}: _(RawOrigin::Signed(curator), dao_id, bounty_id, beneficiary)

	claim_bounty {
		setup_pot_account::<T, I>();
		let (dao_id, origin, curator, bounty_id) = init_bounty::<T, I>()?;
		Treasury::<T, I>::on_initialize(T::BlockNumber::zero());

		let bounty_id = BountyCount::<T, I>::get(dao_id) - 1;

		let beneficiary_account: T::AccountId = account("beneficiary", 0, SEED);
		let beneficiary = beneficiary_account.clone();
		Bounties::<T, I>::award_bounty(RawOrigin::Signed(curator.clone()).into(), dao_id, bounty_id, beneficiary)?;

		// frame_system::Pallet::<T>::set_block_number(T::BountyDepositPayoutDelay::get() + 1u32.into());
		frame_system::Pallet::<T>::set_block_number( 1u32.into());
		ensure!(T::Currency::free_balance(&beneficiary_account).is_zero(), "Beneficiary already has balance");

	}: _(RawOrigin::Signed(curator), dao_id, bounty_id)
	verify {
		ensure!(!T::Currency::free_balance(&beneficiary_account).is_zero(), "Beneficiary didn't get paid");
	}

	close_bounty_active {
		setup_pot_account::<T, I>();
		let (dao_id, origin, curator, bounty_id) = init_bounty::<T, I>()?;
		Treasury::<T, I>::on_initialize(T::BlockNumber::zero());
		let bounty_id = BountyCount::<T, I>::get(dao_id) - 1;
		let approve_origin = T::ApproveOrigin::try_successful_origin(&origin).map_err(|_| BenchmarkError::Weightless)?;
	}: close_bounty<T::RuntimeOrigin>(approve_origin, dao_id, bounty_id)
	verify {
		assert_last_event::<T, I>(Event::BountyCanceled { dao_id, index: bounty_id }.into())
	}

	extend_bounty_expiry {
		setup_pot_account::<T, I>();
		let (dao_id, origin, curator, bounty_id) = init_bounty::<T, I>()?;
		Treasury::<T, I>::on_initialize(T::BlockNumber::zero());

		let bounty_id = BountyCount::<T, I>::get(dao_id) - 1;
	}: _(RawOrigin::Signed(curator), dao_id, bounty_id, Vec::new())
	verify {
		assert_last_event::<T, I>(Event::BountyExtended { dao_id, index: bounty_id, update_due: 10_u32.into() }.into())
	}

	spend_funds {
		let b in 0 .. 100;
		setup_pot_account::<T, I>();
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
