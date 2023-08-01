#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	dispatch::DispatchError,
	pallet_prelude::*,
	traits::{Currency, ExistenceRequirement::KeepAlive, Get, ReservableCurrency},
};
pub use pallet::*;
use scale_info::prelude::*;
use sp_runtime::Saturating;
use sp_std::str;

use dao_primitives::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;

type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

type SubscriptionOf<T> = DaoSubscription<
	<T as frame_system::Config>::BlockNumber,
	VersionedDaoSubscriptionTier,
	VersionedDaoSubscriptionDetails<<T as frame_system::Config>::BlockNumber, BalanceOf<T>>,
>;

type AccountFunctionBalanceOf<T> =
	FunctionPerBlock<<T as frame_system::Config>::BlockNumber, DaoFunctionBalance>;

/// Dao ID. Just a `u32`.
pub type DaoId = u32;

#[frame_support::pallet]
pub mod pallet {
	pub use super::*;
	use crate::weights::WeightInfo;
	use frame_support::PalletId;
	use frame_system::{ensure_root, pallet_prelude::OriginFor};
	use sp_runtime::traits::AccountIdConversion;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(4);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

		/// Chain Treasury Pallet Id - used for deriving its sovereign account ID.
		#[pallet::constant]
		type TreasuryPalletId: Get<PalletId>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::storage]
	#[pallet::getter(fn subscription_tiers)]
	pub(super) type SubscriptionTiers<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		VersionedDaoSubscriptionTier,
		VersionedDaoSubscriptionDetails<T::BlockNumber, BalanceOf<T>>,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn subscriptions)]
	pub(super) type Subscriptions<T: Config> =
		StorageMap<_, Blake2_128Concat, DaoId, SubscriptionOf<T>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn account_function_balances)]
	pub(super) type AccountFunctionBalances<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, AccountFunctionBalanceOf<T>, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		DaoSubscribed {
			dao_id: DaoId,
			subscribed_at: T::BlockNumber,
			until: T::BlockNumber,
			tier: VersionedDaoSubscriptionTier,
			details: VersionedDaoSubscriptionDetails<T::BlockNumber, BalanceOf<T>>,
		},
		DaoSubscriptionExtended {
			dao_id: DaoId,
			status: DaoSubscriptionStatus<T::BlockNumber>,
			fn_balance: DaoFunctionBalance,
		},
		DaoSubscriptionChanged {
			dao_id: DaoId,
			tier: VersionedDaoSubscriptionTier,
			details: VersionedDaoSubscriptionDetails<T::BlockNumber, BalanceOf<T>>,
			status: DaoSubscriptionStatus<T::BlockNumber>,
			fn_balance: DaoFunctionBalance,
		},
		DaoSubscriptionTierUpdated {
			tier: VersionedDaoSubscriptionTier,
			details: VersionedDaoSubscriptionDetails<T::BlockNumber, BalanceOf<T>>,
		},
		DaoSubscriptionSuspended {
			dao_id: DaoId,
			reason: SuspensionReason,
		},
		DaoUnsubscribed {
			dao_id: DaoId,
		},
	}

	#[derive(PartialEq)]
	#[pallet::error]
	pub enum Error<T> {
		AlreadySubscribed,
		SubscriptionNotExists,
		SubscriptionExpired,
		SubscriptionSuspended,
		FunctionBalanceLow,
		InvalidSubscriptionTier,
		NotSupported,
		FunctionDisabled,
		AlreadySuspended,
		TooManyCallsPerBlock,
		TooManyCallsForAccount,
		TooManyMembers,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(T::WeightInfo::set_subscription_tiers())]
		#[pallet::call_index(0)]
		pub fn set_subscription_tier(
			origin: OriginFor<T>,
			tier: VersionedDaoSubscriptionTier,
			details: VersionedDaoSubscriptionDetails<T::BlockNumber, BalanceOf<T>>,
		) -> DispatchResult {
			ensure_root(origin.clone())?;

			Self::do_set_subscription_tier(tier, details)
		}

		#[pallet::weight(T::WeightInfo::suspend_subscription())]
		#[pallet::call_index(1)]
		pub fn suspend_subscription(
			origin: OriginFor<T>,
			dao_id: DaoId,
			reason: SuspensionReason,
		) -> DispatchResult {
			ensure_root(origin.clone())?;

			Subscriptions::<T>::try_mutate(
				dao_id,
				|maybe_subscription| -> Result<(), DispatchError> {
					match maybe_subscription {
						None => Err(Error::<T>::SubscriptionNotExists.into()),
						Some(subscription) => match subscription.status {
							DaoSubscriptionStatus::Active { .. } => {
								subscription.status = DaoSubscriptionStatus::Suspended {
									at: frame_system::Pallet::<T>::block_number(),
									reason: reason.clone(),
								};

								Ok(())
							},
							DaoSubscriptionStatus::Suspended { .. } =>
								Err(Error::<T>::AlreadySuspended.into()),
						},
					}
				},
			)?;

			Self::deposit_event(Event::DaoSubscriptionSuspended { dao_id, reason });

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn do_set_subscription_tier(
			tier: VersionedDaoSubscriptionTier,
			details: VersionedDaoSubscriptionDetails<T::BlockNumber, BalanceOf<T>>,
		) -> Result<(), DispatchError> {
			SubscriptionTiers::<T>::insert(tier.clone(), details.clone());

			Self::deposit_event(Event::DaoSubscriptionTierUpdated { tier, details });

			Ok(())
		}

		/// Ensures if subscription is active and indexes function call
		/// - `dao_id`: DAO ID.
		/// - `extra_check`: Additional check function to call.
		pub fn ensure_active<F>(dao_id: DaoId, extra_check: F) -> Result<(), Error<T>>
		where
			F: FnOnce(
				&mut DaoSubscription<
					T::BlockNumber,
					VersionedDaoSubscriptionTier,
					VersionedDaoSubscriptionDetails<T::BlockNumber, BalanceOf<T>>,
				>,
			) -> Result<(), Error<T>>,
		{
			Subscriptions::<T>::try_mutate(dao_id, |maybe_subscription| -> Result<(), Error<T>> {
				match maybe_subscription {
					None => Err(Error::<T>::SubscriptionNotExists),
					Some(subscription) => {
						let DaoSubscription { status, fn_balance, details, .. } =
							subscription.clone();

						match status {
							DaoSubscriptionStatus::Active { until } => {
								let cur_block = frame_system::Pallet::<T>::block_number();
								ensure!(until >= cur_block, Error::<T>::SubscriptionExpired);

								extra_check(subscription)?;

								subscription.fn_balance = fn_balance
									.checked_sub(1)
									.ok_or(Error::<T>::FunctionBalanceLow)?;

								let (block_number, fn_calls) = subscription.fn_per_block;

								let (block_number, fn_calls) = if block_number == cur_block {
									(block_number, fn_calls.saturating_add(1))
								} else {
									(cur_block, 1_u32)
								};

								match details {
									VersionedDaoSubscriptionDetails::Default(
										DaoSubscriptionDetailsV1 { fn_per_block_limit, .. },
									) => {
										ensure!(
											fn_calls <= fn_per_block_limit,
											Error::<T>::TooManyCallsPerBlock
										)
									},
								}

								subscription.fn_per_block = (block_number, fn_calls);

								Ok(())
							},
							DaoSubscriptionStatus::Suspended { .. } =>
								Err(Error::<T>::SubscriptionSuspended),
						}
					},
				}
			})
		}

		pub fn treasury_account_id() -> T::AccountId {
			T::TreasuryPalletId::get().into_account_truncating()
		}

		// TODO: try to use default for subscription details
		pub fn get_default_tier_details() -> (
			VersionedDaoSubscriptionTier,
			VersionedDaoSubscriptionDetails<T::BlockNumber, BalanceOf<T>>,
		) {
			let tier = VersionedDaoSubscriptionTier::Default(DaoSubscriptionTierV1::Basic);
			let details = VersionedDaoSubscriptionDetails::Default(DaoSubscriptionDetailsV1 {
				duration: MONTH_IN_BLOCKS.into(),
				price: TryInto::<BalanceOf<T>>::try_into(DEFAULT_SUBSCRIPTION_PRICE).ok().unwrap(),
				fn_call_limit: DEFAULT_FUNCTION_CALL_LIMIT,
				fn_per_block_limit: DEFAULT_FUNCTION_PER_BLOCK_LIMIT,
				max_members: DEFAULT_MEMBER_COUNT_LIMIT,
				dao: Some(DaoPalletSubscriptionDetailsV1 {
					update_dao_metadata: true,
					update_dao_policy: true,
					mint_dao_token: true,
				}),
				bounties: Some(BountiesSubscriptionDetailsV1 {
					create_bounty: true,
					propose_curator: true,
					unassign_curator: true,
					accept_curator: true,
					award_bounty: true,
					claim_bounty: true,
					close_bounty: true,
					extend_bounty_expiry: true,
				}),
				council: Some(CollectiveSubscriptionDetailsV1 {
					propose: true,
					vote: true,
					close: true,
				}),
				council_membership: Some(MembershipSubscriptionDetailsV1 {
					add_member: true,
					remove_member: true,
					swap_member: true,
					change_key: true,
				}),
				tech_committee: Some(CollectiveSubscriptionDetailsV1 {
					propose: false,
					vote: false,
					close: false,
				}),
				tech_committee_membership: Some(MembershipSubscriptionDetailsV1 {
					add_member: true,
					remove_member: true,
					swap_member: true,
					change_key: false,
				}),
				democracy: Some(DemocracySubscriptionDetailsV1 {
					propose: false,
					second: false,
					vote: false,
					delegate: false,
					undelegate: false,
					unlock: false,
					remove_vote: false,
					remove_other_vote: false,
				}),
				treasury: Some(TreasurySubscriptionDetailsV1 { spend: true, transfer_token: true }),
			});

			(tier, details)
		}
	}
}

impl<T: Config>
	DaoSubscriptionProvider<
		DaoId,
		T::AccountId,
		T::BlockNumber,
		VersionedDaoSubscriptionTier,
		VersionedDaoSubscriptionDetails<T::BlockNumber, BalanceOf<T>>,
	> for Pallet<T>
{
	fn subscribe(
		dao_id: DaoId,
		account_id: &T::AccountId,
		tier: Option<VersionedDaoSubscriptionTier>,
	) -> Result<(), DispatchError> {
		ensure!(Subscriptions::<T>::get(dao_id).is_none(), Error::<T>::AlreadySubscribed);

		// Note: Should only be used for benchmarking.
		#[cfg(feature = "runtime-benchmarks")]
		match tier.clone() {
			None => {},
			Some(tier) => match tier {
				VersionedDaoSubscriptionTier::Default(tier) => match tier {
					DaoSubscriptionTierV1::NoTier => return Ok(()),
					_ => {},
				},
			},
		}

		let subscribed_at = frame_system::Pallet::<T>::block_number();

		let (default_tier, default_details) = Self::get_default_tier_details();
		let tier = tier.unwrap_or(default_tier);
		let details = SubscriptionTiers::<T>::get(tier.clone()).map_or(default_details, |t| t);
		let (until, fn_call_limit, price) = match details {
			VersionedDaoSubscriptionDetails::Default(DaoSubscriptionDetailsV1 {
				duration,
				price,
				fn_call_limit,
				..
			}) => (subscribed_at.saturating_add(duration), fn_call_limit, price),
		};

		T::Currency::transfer(account_id, &Self::treasury_account_id(), price, KeepAlive)?;

		let subscription: DaoSubscription<
			T::BlockNumber,
			VersionedDaoSubscriptionTier,
			VersionedDaoSubscriptionDetails<T::BlockNumber, BalanceOf<T>>,
		> = DaoSubscription {
			tier: tier.clone(),
			details: details.clone(),
			subscribed_at,
			last_renewed_at: None,
			status: DaoSubscriptionStatus::Active { until },
			fn_balance: fn_call_limit,
			fn_per_block: (subscribed_at, 0_u32),
		};

		Subscriptions::<T>::insert(dao_id, subscription);

		Self::deposit_event(Event::DaoSubscribed { dao_id, subscribed_at, until, tier, details });

		Ok(())
	}

	fn unsubscribe(dao_id: DaoId) -> Result<(), DispatchError> {
		let subscription = Subscriptions::<T>::get(dao_id);
		if subscription.is_some() {
			Subscriptions::<T>::remove(dao_id);

			Self::deposit_event(Event::DaoUnsubscribed { dao_id });
		}

		Ok(())
	}

	fn extend_subscription(dao_id: DaoId, account_id: &T::AccountId) -> Result<(), DispatchError> {
		let (status, fn_balance) = Subscriptions::<T>::try_mutate(
			dao_id,
			|maybe_subscription| -> Result<
				(DaoSubscriptionStatus<T::BlockNumber>, DaoFunctionBalance),
				DispatchError,
			> {
				match maybe_subscription {
					None => Err(Error::<T>::SubscriptionNotExists.into()),
					Some(subscription) => {
						let (_, default_details) = Self::get_default_tier_details();
						let details = SubscriptionTiers::<T>::get(subscription.tier.clone())
							.map_or(default_details, |t| t);

						let (duration, fn_call_limit, price) = match subscription.details {
							VersionedDaoSubscriptionDetails::Default(
								DaoSubscriptionDetailsV1 { duration, price, fn_call_limit, .. },
							) => (duration, fn_call_limit, price),
						};

						T::Currency::transfer(
							account_id,
							&Self::treasury_account_id(),
							price,
							KeepAlive,
						)?;

						let cur_block = frame_system::Pallet::<T>::block_number();

						let until = match subscription.status {
							// overriding `until` based on the current active value
							DaoSubscriptionStatus::Active { until } => until + duration,
							DaoSubscriptionStatus::Suspended { .. } => cur_block + duration,
						};

						let (status, fn_balance) = (
							DaoSubscriptionStatus::Active { until },
							subscription.fn_balance.saturating_add(fn_call_limit),
						);

						subscription.status = status.clone();
						subscription.last_renewed_at = Some(cur_block);
						subscription.fn_balance = fn_balance;
						subscription.details = details;

						Ok((status, fn_balance))
					},
				}
			},
		)?;

		Self::deposit_event(Event::DaoSubscriptionExtended { dao_id, status, fn_balance });

		Ok(())
	}

	fn change_subscription_tier(
		dao_id: DaoId,
		account_id: &T::AccountId,
		tier: VersionedDaoSubscriptionTier,
	) -> Result<(), DispatchError> {
		let cur_block = frame_system::Pallet::<T>::block_number();

		let details =
			SubscriptionTiers::<T>::get(tier.clone()).ok_or(Error::<T>::InvalidSubscriptionTier)?;

		let (status, fn_balance) = Subscriptions::<T>::try_mutate(
			dao_id,
			|maybe_subscription| -> Result<
				(DaoSubscriptionStatus<T::BlockNumber>, DaoFunctionBalance),
				DispatchError,
			> {
				match maybe_subscription {
					None => Err(Error::<T>::SubscriptionNotExists.into()),
					Some(subscription) => {
						let (duration, fn_call_limit, price) = match details {
							VersionedDaoSubscriptionDetails::Default(
								DaoSubscriptionDetailsV1 { duration, fn_call_limit, price, .. },
							) => (duration, fn_call_limit, price),
						};

						T::Currency::transfer(
							account_id,
							&Self::treasury_account_id(),
							price,
							KeepAlive,
						)?;

						subscription.tier = tier.clone();
						subscription.details = details.clone();

						let until = match subscription.status {
							DaoSubscriptionStatus::Active { until } =>
								until.saturating_add(duration),
							DaoSubscriptionStatus::Suspended { .. } => duration,
						};

						let (status, fn_balance) = (
							DaoSubscriptionStatus::Active { until },
							subscription.fn_balance.saturating_add(fn_call_limit),
						);

						subscription.status = status.clone();
						subscription.last_renewed_at = Some(cur_block);
						subscription.fn_balance = fn_balance;

						Ok((status, fn_balance))
					},
				}
			},
		)?;

		Self::deposit_event(Event::DaoSubscriptionChanged {
			dao_id,
			tier,
			details,
			status,
			fn_balance,
		});

		Ok(())
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn assign_subscription_tier(tier: VersionedDaoSubscriptionTier) -> DispatchResult {
		let (_, details) = Self::get_default_tier_details();
		Self::do_set_subscription_tier(tier, details)
	}
}

impl<T: Config> AccountFnCallRateLimiter<T::AccountId, DaoFunctionBalance> for Pallet<T> {
	fn ensure_limited(
		account_id: &T::AccountId,
		limit: DaoFunctionBalance,
	) -> Result<(), DispatchError> {
		let cur_block = frame_system::Pallet::<T>::block_number();

		let (block_number, fn_balance) = AccountFunctionBalances::<T>::get(account_id.clone())
			.map_or_else(
				|| (cur_block, 1),
				|(block_number, fn_calls)| {
					if block_number == cur_block {
						(block_number, fn_calls.saturating_add(1))
					} else {
						(cur_block, 1_u32)
					}
				},
			);

		ensure!(fn_balance <= limit, Error::<T>::TooManyCallsForAccount);

		AccountFunctionBalances::<T>::insert(account_id, (block_number, fn_balance));

		Ok(())
	}
}
