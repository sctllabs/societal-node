//! Benchmarking setup for pallet-dao

use super::*;

#[allow(unused)]
use crate::Pallet as Dao;
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_system::RawOrigin;

const SEED: u32 = 0;

// Create the pre-requisite information needed to create a dao.
fn setup_dao<T: Config>(
	u: u32,
) -> (T::AccountId, Vec<<T::Lookup as StaticLookup>::Source>, Vec<u8>) {
	let caller = account("caller", u, SEED);
	let council_account = account("account", u, SEED);
	let value: BalanceOf<T> = 100u32.into();
	let _ = T::Currency::make_free_balance_be(&caller, value);
	(caller, vec![council_account], vec![0, 1])
}

benchmarks! {
	create_dao {
		let (caller, council, data) = setup_dao::<T>(0);
	}: _(RawOrigin::Signed(caller), council, data)
	verify {
		assert_eq!(NextDaoId::<T>::get(), 0);
	}

	impl_benchmark_test_suite!(Dao, crate::mock::new_test_ext(), crate::mock::Test);
}
