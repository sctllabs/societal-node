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
	Subscription::<T>::subscribe(0, &caller, None)
}

benchmarks! {

	set_subscription_tier {
		let (tier, details) = Subscription::<T>::get_default_tier_details();
	}: _(RawOrigin::Root, tier.clone(), details.clone())
	verify {
		let subscription_tier = Subscription::<T>::subscription_tiers(tier).unwrap();
		assert_eq!(
			subscription_tier,
			details
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
