//! Benchmarking setup for pallet-dao

use super::*;

#[allow(unused)]
use crate::Pallet as Dao;
use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;

use serde_json::json;

const SEED: u32 = 0;

// Create the pre-requisite information needed to create a dao.
fn setup_dao<T: Config>(
	u: u32,
) -> (T::AccountId, Vec<<T::Lookup as StaticLookup>::Source>, Vec<u8>) {
	let caller = account("caller", u, SEED);
	let council_account = account("account", u, SEED);
	let value: BalanceOf<T> = 100u32.into();
	let _ = T::Currency::make_free_balance_be(&caller, value);

	let dao_json = json!({
		"name": "name",
		"purpose": "purpose",
		"metadata": "metadata",
		"policy": {
			"proposal_bond": 1,
			"proposal_bond_min": 1,
			"proposal_period": 300000,
			"approve_origin": [
				1,
				2
			],
			"reject_origin": [
				1,
				2
			]
		},
		"token": {
			"token_id": 1,
			"min_balance": "10000000",
			"metadata": {
				"name": "token",
				"symbol": "symbol",
				"decimals": 2
			}
		}
	});

	(caller, vec![council_account], serde_json::to_vec(&dao_json).ok().unwrap())
}

benchmarks! {
	create_dao {
		let (caller, council, data) = setup_dao::<T>(0);
	}: _(RawOrigin::Signed(caller), council, data)
	verify {
		assert_eq!(NextDaoId::<T>::get(), 1);
	}

	impl_benchmark_test_suite!(Dao, crate::mock::new_test_ext(), crate::mock::Test);
}
