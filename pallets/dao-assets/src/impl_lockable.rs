//! Assets pallet's `LockableAsset` implementation.

use super::*;
use frame_support::traits::LockIdentifier;
use sp_runtime::traits::MaybeSerializeDeserialize;
use sp_std::fmt::Debug;

impl<T: Config<I>, I: 'static> LockableAsset<T::AccountId> for Pallet<T, I>
where
	T::Balance: MaybeSerializeDeserialize + Debug,
{
	type Moment = T::BlockNumber;

	type MaxLocks = T::MaxLocks;

	/// Set a lock on the `asset` balance of `who`.
	fn set_lock(id: LockIdentifier, asset: &Self::AssetId, who: &T::AccountId, amount: T::Balance) {
		if amount.is_zero() {
			return
		}
		let mut new_lock = Some(AssetBalanceLock { id, amount });
		let mut locks = Self::locks(asset, who)
			.into_iter()
			.filter_map(|l| if l.id == id { new_lock.take() } else { Some(l) })
			.collect::<Vec<_>>();
		if let Some(lock) = new_lock {
			locks.push(lock)
		}
		Self::update_locks(asset, who, &locks[..]);
	}

	/// Extend a lock on the `asset` balance of `who`.
	fn extend_lock(
		id: LockIdentifier,
		asset: &Self::AssetId,
		who: &T::AccountId,
		amount: T::Balance,
	) {
		if amount.is_zero() {
			return
		}
		let mut new_lock = Some(AssetBalanceLock { id, amount });
		let mut locks = Self::locks(asset, who)
			.into_iter()
			.filter_map(|l| {
				if l.id == id {
					new_lock
						.take()
						.map(|nl| AssetBalanceLock { id: l.id, amount: l.amount.max(nl.amount) })
				} else {
					Some(l)
				}
			})
			.collect::<Vec<_>>();
		if let Some(lock) = new_lock {
			locks.push(lock)
		}
		Self::update_locks(asset, who, &locks[..]);
	}

	fn remove_lock(id: LockIdentifier, asset: &Self::AssetId, who: &T::AccountId) {
		let mut locks = Self::locks(asset, who);
		locks.retain(|l| l.id != id);
		Self::update_locks(asset, who, &locks[..]);
	}
}

impl<T: Config<I>, I: 'static> FrozenBalance<T::AssetId, T::AccountId, T::Balance>
	for Pallet<T, I>
{
	fn frozen_balance(asset: T::AssetId, who: &T::AccountId) -> Option<T::Balance> {
		match Account::<T, I>::get(asset, who).map(|acc| acc.frozen_balance).or(None) {
			Some(frozen_balance) if !frozen_balance.is_zero() => Some(frozen_balance),
			_ => None,
		}
	}

	fn died(asset: T::AssetId, who: &T::AccountId) {
		// Sanity check: dead accounts have no balance.
		assert!(Self::balance(asset, who.clone()).is_zero());
	}
}
