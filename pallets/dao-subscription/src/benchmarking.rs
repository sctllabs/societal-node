//! Benchmarking setup for pallet-dao

use super::*;

use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;

use crate::Pallet as Subscription;
use sp_runtime::traits::Bounded;

const SEED: u32 = 0;

fn setup_subscription<T: Config>() -> Result<(), DispatchError> {
	let caller: T::AccountId = account("caller", 0, SEED);
	T::Currency::make_free_balance_be(&caller, BalanceOf::<T>::max_value() / 2u32.into());
	Subscription::<T>::subscribe(0, &caller)
}

benchmarks! {

	set_subscription_tiers {
		let duration = MONTH_IN_BLOCKS.into();
		let price = TryInto::<BalanceOf<T>>::try_into(DEFAULT_SUBSCRIPTION_PRICE).ok().unwrap();
		let fn_call_limit = DEFAULT_FUNCTION_CALL_LIMIT;
		let fn_per_block_limit = DEFAULT_FUNCTION_PER_BLOCK_LIMIT;

		let tier = VersionedDaoSubscription::Default(DaoSubscriptionTiersV1::Basic {
			duration,
			price,
			fn_call_limit,
			fn_per_block_limit,
		});
	}: _(RawOrigin::Root, tier)
	verify {
		let subscription_tiers = Subscription::<T>::subscription_tiers().unwrap();
		assert_eq!(
			subscription_tiers,
			VersionedDaoSubscription::Default(DaoSubscriptionTiersV1::Basic {
				duration,
				price,
				fn_call_limit,
				fn_per_block_limit,
			})
		);
	}

	suspend_subscription {
		setup_subscription::<T>()?;
	}: _(RawOrigin::Root, 0, SuspensionReason::Other)
	verify {
		let DaoSubscription { status, .. } = Subscription::<T>::subscriptions(0).unwrap();
		assert_eq!(status,
			DaoSubscriptionStatus::Suspended { reason: SuspensionReason::Other, at: 1_u32.into() });
	}

	impl_benchmark_test_suite!(Subscription, crate::mock::new_test_ext(), crate::mock::Test);
}
