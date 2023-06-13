// Changes made comparing to the original benchmarking of the FRAME pallet-treasury:
// - using DAO as a parameter for pallet functions

//! DAO Treasury pallet benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

#[allow(unused)]
use crate::Pallet as Treasury;

use dao_primitives::DaoPolicyProportion;
use frame_benchmarking::{account, benchmarks_instance_pallet, BenchmarkError};
use frame_support::dispatch::UnfilteredDispatchable;
use serde_json::json;

const SEED: u32 = 0;

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

fn assert_last_event<T: Config<I>, I: 'static>(generic_event: <T as Config<I>>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

benchmarks_instance_pallet! {
	spend {
		create_dao::<T, I>()?;
		let (_, value, beneficiary_lookup) = setup_proposal::<T, _>(SEED);
		let origin = get_dao_origin::<T, I>(0)?;
		let beneficiary = T::Lookup::lookup(beneficiary_lookup.clone()).unwrap();
		let call = Call::<T, I>::spend { dao_id: 0, amount: value, beneficiary: beneficiary_lookup };
	}: {
		call.dispatch_bypass_filter(origin)?;
	}
	verify {
		assert_last_event::<T, I>(
			Event::SpendApproved {
				dao_id: 0,
				proposal_index: 0,
				amount: value,
				beneficiary
			}.into()
		)
	}

	transfer_token {
		create_dao::<T, I>()?;
		let (_, value, beneficiary_lookup) = setup_proposal::<T, _>(SEED);
		let origin = get_dao_origin::<T, I>(0)?;
		let amount = 100_u32.into();
		let from = T::DaoProvider::dao_account_id(0);
		let to = T::Lookup::lookup(beneficiary_lookup.clone()).unwrap();
	}: _<T::RuntimeOrigin>(origin, 0, amount, beneficiary_lookup)
	verify { }

	impl_benchmark_test_suite!(Treasury, crate::tests::new_test_ext(), crate::tests::Test);
}
