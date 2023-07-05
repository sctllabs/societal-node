use super::*;
use crate::mock::*;
use frame_support::{assert_noop, assert_ok};
use pallet_balances::Error as BalancesError;

#[test]
fn subscribe_not_enough_balance() {
	new_test_ext().execute_with(|| {
		assert_noop!(Subscription::subscribe(0, &1), BalancesError::<Test>::InsufficientBalance);
	})
}

#[test]
fn subscribe_works() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&1, DEFAULT_SUBSCRIPTION_PRICE.saturating_mul(2_u32.into()));

		fast_forward_to(1);

		assert_ok!(Subscription::subscribe(0, &1));

		assert_eq!(Balances::free_balance(&1), DEFAULT_SUBSCRIPTION_PRICE);

		let tier = VersionedDaoSubscription::Default(DaoSubscriptionTiersV1::Basic {
			duration: MONTH_IN_BLOCKS.into(),
			price: DEFAULT_SUBSCRIPTION_PRICE,
			fn_call_limit: DEFAULT_FUNCTION_CALL_LIMIT,
			fn_per_block_limit: DEFAULT_FUNCTION_PER_BLOCK_LIMIT,
		});

		assert_eq!(
			Subscription::subscriptions(0),
			Some(DaoSubscription {
				subscribed_at: 1_u32.into(),
				last_renewed_at: None,
				tier: tier.clone(),
				status: DaoSubscriptionStatus::Active {
					until: MONTH_IN_BLOCKS.saturating_add(1_u32).into()
				},
				fn_balance: DEFAULT_FUNCTION_CALL_LIMIT,
				fn_per_block: (1_u32.into(), 0),
			})
		);

		System::assert_last_event(
			crate::Event::DaoSubscribed {
				dao_id: 0,
				subscribed_at: 1,
				until: (MONTH_IN_BLOCKS + 1_u32).into(),
				tier,
			}
			.into(),
		);
	})
}

#[test]
fn fails_already_subscribed() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&1, DEFAULT_SUBSCRIPTION_PRICE.saturating_mul(2_u32.into()));

		assert_ok!(Subscription::subscribe(0, &1));

		assert_noop!(Subscription::subscribe(0, &1), Error::<Test>::AlreadySubscribed);
	})
}

#[test]
fn ensure_active_works() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&1, DEFAULT_SUBSCRIPTION_PRICE.saturating_mul(2_u32.into()));

		assert_ok!(Subscription::subscribe(0, &1));

		assert_ok!(Subscription::ensure_active(0));
	})
}

#[test]
fn ensure_active_not_exists() {
	new_test_ext().execute_with(|| {
		assert_noop!(Subscription::ensure_active(0), Error::<Test>::SubscriptionNotExists);
	})
}

#[test]
fn ensure_active_expired() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&1, DEFAULT_SUBSCRIPTION_PRICE.saturating_mul(2_u32.into()));

		assert_ok!(Subscription::subscribe(0, &1));

		fast_forward_to((MONTH_IN_BLOCKS + 1).into());

		assert_noop!(Subscription::ensure_active(0), Error::<Test>::SubscriptionExpired);
	})
}

#[test]
fn ensure_active_fn_limit_exceeded() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&1, DEFAULT_SUBSCRIPTION_PRICE.saturating_mul(2_u32.into()));

		assert_ok!(Subscription::subscribe(0, &1));

		let mut n = 0;
		while n < DEFAULT_FUNCTION_CALL_LIMIT {
			assert_ok!(Subscription::ensure_active(0));

			next_block();
			n += 1;
		}

		assert_noop!(Subscription::ensure_active(0), Error::<Test>::FunctionBalanceLow);
	})
}

#[test]
fn extend_subscription_balance_low() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&1, DEFAULT_SUBSCRIPTION_PRICE.saturating_mul(2_u32.into()));

		assert_ok!(Subscription::subscribe(0, &1));

		fast_forward_to((MONTH_IN_BLOCKS + 1).into());

		assert_noop!(
			Subscription::extend_subscription(0, 2),
			BalancesError::<Test>::InsufficientBalance
		);
	})
}

#[test]
fn extend_subscription_works() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&1, DEFAULT_SUBSCRIPTION_PRICE.saturating_mul(3_u32.into()));

		assert_ok!(Subscription::subscribe(0, &1));

		fast_forward_to((MONTH_IN_BLOCKS + 1).into());

		assert_ok!(Subscription::extend_subscription(0, 1));

		let tier = VersionedDaoSubscription::Default(DaoSubscriptionTiersV1::Basic {
			duration: MONTH_IN_BLOCKS.into(),
			price: DEFAULT_SUBSCRIPTION_PRICE,
			fn_call_limit: DEFAULT_FUNCTION_CALL_LIMIT,
			fn_per_block_limit: DEFAULT_FUNCTION_PER_BLOCK_LIMIT,
		});

		assert_eq!(
			Subscription::subscriptions(0),
			Some(DaoSubscription {
				subscribed_at: 0_u32.into(),
				last_renewed_at: Some(MONTH_IN_BLOCKS.saturating_add(1).into()),
				tier: tier.clone(),
				status: DaoSubscriptionStatus::Active {
					until: MONTH_IN_BLOCKS.saturating_mul(2).into()
				},
				fn_balance: DEFAULT_FUNCTION_CALL_LIMIT.saturating_mul(2),
				fn_per_block: (0_u32.into(), 0)
			})
		);
	})
}

fn next_block() {
	System::set_block_number(System::block_number() + 1);
}

fn fast_forward_to(n: u64) {
	while System::block_number() < n {
		next_block();
	}
}
