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
	VersionedDaoSubscription<<T as frame_system::Config>::BlockNumber, BalanceOf<T>>,
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
	pub(super) type SubscriptionTiers<T: Config> =
		StorageValue<_, VersionedDaoSubscription<T::BlockNumber, BalanceOf<T>>, OptionQuery>;

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
			tier: VersionedDaoSubscription<T::BlockNumber, BalanceOf<T>>,
		},
		DaoSubscriptionExtended {
			dao_id: DaoId,
			status: DaoSubscriptionStatus<T::BlockNumber>,
			fn_balance: DaoFunctionBalance,
		},
		DaoSubscriptionTiersUpdated {
			tiers: VersionedDaoSubscription<T::BlockNumber, BalanceOf<T>>,
		},
		DaoSubscriptionSuspended {
			dao_id: DaoId,
			reason: SuspensionReason,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		AlreadySubscribed,
		SubscriptionNotExists,
		SubscriptionExpired,
		SubscriptionSuspended,
		FunctionBalanceLow,
		InvalidSubscriptionTiers,
		NotSupported,
		AlreadySuspended,
		TooManyCallsPerBlock,
		TooManyCallsForAccount,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(T::WeightInfo::set_subscription_tiers())]
		#[pallet::call_index(0)]
		pub fn set_subscription_tiers(
			origin: OriginFor<T>,
			tiers: VersionedDaoSubscription<T::BlockNumber, BalanceOf<T>>,
		) -> DispatchResult {
			ensure_root(origin.clone())?;

			SubscriptionTiers::<T>::put(tiers.clone());

			Self::deposit_event(Event::DaoSubscriptionTiersUpdated { tiers });

			Ok(())
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
		pub fn do_ensure_active(dao_id: DaoId) -> Result<(), DispatchError> {
			Subscriptions::<T>::try_mutate(
				dao_id,
				|maybe_subscription| -> Result<(), DispatchError> {
					match maybe_subscription {
						None => Err(Error::<T>::SubscriptionNotExists.into()),
						Some(subscription) => {
							let DaoSubscription { status, fn_balance, tier, .. } =
								subscription.clone();

							match status {
								DaoSubscriptionStatus::Active { until } => {
									let cur_block = frame_system::Pallet::<T>::block_number();
									ensure!(until >= cur_block, Error::<T>::SubscriptionExpired);

									subscription.fn_balance = fn_balance
										.checked_sub(1)
										.ok_or(Error::<T>::FunctionBalanceLow)?;

									let (block_number, fn_calls) = subscription.fn_per_block;

									let (block_number, fn_calls) = if block_number == cur_block {
										(block_number, fn_calls.saturating_add(1))
									} else {
										(cur_block, 1_u32)
									};

									match tier {
										VersionedDaoSubscription::Default(
											DaoSubscriptionTiersV1::Basic {
												fn_per_block_limit,
												..
											},
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
									Err(Error::<T>::SubscriptionSuspended.into()),
							}
						},
					}
				},
			)
		}

		pub fn do_ensure_limited(
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

		pub fn treasury_account_id() -> T::AccountId {
			T::TreasuryPalletId::get().into_account_truncating()
		}

		pub fn get_default_subscription_tier(
		) -> VersionedDaoSubscription<T::BlockNumber, BalanceOf<T>> {
			VersionedDaoSubscription::Default(DaoSubscriptionTiersV1::Basic {
				duration: MONTH_IN_BLOCKS.into(),
				price: TryInto::<BalanceOf<T>>::try_into(DEFAULT_SUBSCRIPTION_PRICE).ok().unwrap(),
				fn_call_limit: DEFAULT_FUNCTION_CALL_LIMIT,
				fn_per_block_limit: DEFAULT_FUNCTION_PER_BLOCK_LIMIT,
			})
		}
	}
}

impl<T: Config> DaoSubscriptionProvider<DaoId, T::AccountId, T::BlockNumber> for Pallet<T> {
	fn subscribe(dao_id: DaoId, account_id: &T::AccountId) -> Result<(), DispatchError> {
		ensure!(Subscriptions::<T>::get(dao_id).is_none(), Error::<T>::AlreadySubscribed);

		let subscribed_at = frame_system::Pallet::<T>::block_number();
		let tier =
			SubscriptionTiers::<T>::get().map_or(Self::get_default_subscription_tier(), |t| t);
		let (until, fn_call_limit, price) = match tier {
			VersionedDaoSubscription::Default(DaoSubscriptionTiersV1::Basic {
				duration,
				price,
				fn_call_limit,
				..
			}) => (subscribed_at.saturating_add(duration), fn_call_limit, price),
		};

		T::Currency::transfer(account_id, &Self::treasury_account_id(), price, KeepAlive)?;

		let subscription: DaoSubscription<
			T::BlockNumber,
			VersionedDaoSubscription<T::BlockNumber, BalanceOf<T>>,
		> = DaoSubscription {
			subscribed_at,
			last_renewed_at: None,
			tier: tier.clone(),
			status: DaoSubscriptionStatus::Active { until },
			fn_balance: fn_call_limit,
			fn_per_block: (subscribed_at, 0_u32),
		};

		Subscriptions::<T>::insert(dao_id, subscription);

		Self::deposit_event(Event::DaoSubscribed { dao_id, subscribed_at, until, tier });

		Ok(())
	}

	fn extend_subscription(dao_id: DaoId, account_id: T::AccountId) -> Result<(), DispatchError> {
		let tier =
			SubscriptionTiers::<T>::get().map_or(Self::get_default_subscription_tier(), |t| t);

		let (status, fn_balance) = Subscriptions::<T>::try_mutate(
			dao_id,
			|maybe_subscription| -> Result<
				(DaoSubscriptionStatus<T::BlockNumber>, DaoFunctionBalance),
				DispatchError,
			> {
				match maybe_subscription {
					None => Err(Error::<T>::SubscriptionNotExists.into()),
					Some(subscription) => {
						let (duration, fn_call_limit, price) = match tier {
							VersionedDaoSubscription::Default(DaoSubscriptionTiersV1::Basic {
								duration,
								price,
								fn_call_limit,
								..
							}) => (duration, fn_call_limit, price),
						};

						T::Currency::transfer(
							&account_id,
							&Self::treasury_account_id(),
							price,
							KeepAlive,
						)?;

						let cur_block = frame_system::Pallet::<T>::block_number();

						let mut until = cur_block + duration;
						if let DaoSubscriptionStatus::Active { until: cur_until } =
							subscription.status
						{
							// overriding `until` based on the current active value
							until = cur_until + duration;
						}

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

		Self::deposit_event(Event::DaoSubscriptionExtended { dao_id, status, fn_balance });

		Ok(())
	}

	fn ensure_active(dao_id: DaoId) -> Result<(), DispatchError> {
		Self::do_ensure_active(dao_id)
	}
}

impl<T: Config> AccountFnCallRateLimiter<T::AccountId, DaoFunctionBalance> for Pallet<T> {
	fn ensure_limited(
		account_id: &T::AccountId,
		limit: DaoFunctionBalance,
	) -> Result<(), DispatchError> {
		Self::do_ensure_limited(account_id, limit)
	}
}
