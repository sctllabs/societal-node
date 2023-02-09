// Changes made comparing to the original benchmarking of the FRAME pallet-treasury:
// - using DAO as a parameter for pallet functions

//! DAO Treasury pallet benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::{Pallet as Treasury, *};

use dao_primitives::DaoPolicyProportion;
use frame_benchmarking::{account, benchmarks_instance_pallet};
use frame_support::{dispatch::UnfilteredDispatchable, ensure};
use frame_system::RawOrigin;

const SEED: u32 = 0;

// Create the pre-requisite information needed to create a treasury `propose_spend`.
fn setup_proposal<T: Config<I>, I: 'static>(
	u: u32,
) -> (T::AccountId, BalanceOf<T, I>, <T::Lookup as StaticLookup>::Source) {
	let caller = account("caller", u, SEED);
	let value: BalanceOf<T, I> = 100u32.into();
	let _ = T::Currency::make_free_balance_be(&caller, value);
	let beneficiary = account("beneficiary", u, SEED);
	let beneficiary_lookup = T::Lookup::unlookup(beneficiary);
	(caller, value, beneficiary_lookup)
}

// Create proposals that are approved for use in `on_initialize`.
fn create_approved_proposals<T: Config<I>, I: 'static>(n: u32) -> Result<(), &'static str> {
	for i in 0..n {
		let (_caller, value, lookup) = setup_proposal::<T, I>(i);
		Treasury::<T, I>::spend(RawOrigin::Root.into(), 0, value, lookup)?;
	}
	ensure!(<Approvals<T, I>>::get(0).len() == n as usize, "Not all approved");
	Ok(())
}

fn setup_pot_account<T: Config<I>, I: 'static>() {
	let pot_account = Treasury::<T, I>::account_id();
	let value = T::Currency::minimum_balance().saturating_mul(1_000_000_000u32.into());
	let _ = T::Currency::make_free_balance_be(&pot_account, value);
}

fn assert_last_event<T: Config<I>, I: 'static>(generic_event: <T as Config<I>>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

benchmarks_instance_pallet! {
	// This benchmark is short-circuited if `ApproveOrigin` cannot provide
	// a successful origin, in which case `spend` is un-callable and can use weight=0.
	spend {
		let (_, value, beneficiary_lookup) = setup_proposal::<T, _>(SEED);
		let origin = T::ApproveOrigin::try_successful_origin(&DaoOrigin {
			dao_account_id: account("member", 0, SEED),
			proportion: DaoPolicyProportion::AtLeast((1, 1)),
		});
		let beneficiary = T::Lookup::lookup(beneficiary_lookup.clone()).unwrap();
		let call = Call::<T, I>::spend { dao_id: 0, amount: value, beneficiary: beneficiary_lookup };
	}: {
		if let Ok(origin) = origin.clone() {
			call.dispatch_bypass_filter(origin)?;
		}
	}
	verify {
		if origin.is_ok() {
			assert_last_event::<T, I>(Event::SpendApproved { dao_id: 0, proposal_index: 0, amount: value, beneficiary }.into())
		}
	}

	on_initialize_proposals {
		let p in 0 .. T::MaxApprovals::get();
		setup_pot_account::<T, _>();
		create_approved_proposals::<T, _>(p)?;
	}: {
		<Treasury::<T, _> as Hooks<T::BlockNumber>>::on_initialize(T::BlockNumber::zero());
	}

	impl_benchmark_test_suite!(Treasury, crate::tests::new_test_ext(), crate::tests::Test);
}
