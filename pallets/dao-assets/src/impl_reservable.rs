use super::*;
use frame_support::traits::{
	fungibles::{Inspect, InspectHold, MutateHold},
	DefensiveSaturating,
};
use sp_runtime::traits::Zero;
use sp_std::cmp;

impl<T: Config<I>, I: 'static> InspectHold<T::AccountId> for Pallet<T, I> {
	fn balance_on_hold(asset: Self::AssetId, who: &T::AccountId) -> Self::Balance {
		Account::<T, I>::get(asset, who)
			.map(|acc| acc.reserved_balance)
			.unwrap_or_default()
	}

	fn can_hold(asset: Self::AssetId, who: &T::AccountId, amount: Self::Balance) -> bool {
		if amount.is_zero() {
			return true
		}

		let balance = Account::<T, I>::get(asset, who).map(|acc| acc.balance).unwrap_or_default();

		balance.checked_sub(&amount).map_or(false, |new_balance| {
			new_balance >= Self::reducible_balance(asset, who, true).unwrap_or_default()
		})
	}
}

impl<T: Config<I>, I: 'static> MutateHold<T::AccountId> for Pallet<T, I> {
	fn hold(asset: Self::AssetId, who: &T::AccountId, amount: Self::Balance) -> DispatchResult {
		if amount.is_zero() {
			return Ok(())
		}

		Account::<T, I>::try_mutate(asset, who, |maybe_details| -> DispatchResult {
			match maybe_details {
				None => Err(Error::<T, I>::NoAccount.into()),
				Some(account) => {
					account.balance =
						account.balance.checked_sub(&amount).ok_or(Error::<T, I>::BalanceLow)?;

					account.reserved_balance = account
						.reserved_balance
						.checked_add(&amount)
						.ok_or(ArithmeticError::Overflow)?;

					ensure!(
						Self::can_withdraw(asset, who, account.balance) ==
							WithdrawConsequence::Success,
						Error::<T, I>::LiquidityRestrictions
					);

					Ok(())
				},
			}
		})
	}

	fn release(
		asset: Self::AssetId,
		who: &T::AccountId,
		amount: Self::Balance,
		best_effort: bool,
	) -> Result<Self::Balance, DispatchError> {
		if amount.is_zero() {
			return Ok(Zero::zero())
		}

		if Self::balance_on_hold(asset, who).is_zero() {
			return Ok(amount)
		}

		let actual = match Account::<T, I>::try_mutate(asset, who, |maybe_details| {
			match maybe_details {
				None => Err::<T::Balance, Error<T, I>>(Error::<T, I>::NoAccount),
				Some(account) => {
					let actual = cmp::min(account.reserved_balance, amount);
					ensure!(best_effort || actual >= amount, Error::<T, I>::BalanceLow);

					let conseq = Self::can_release(asset, who, actual).into_result();
					ensure!(conseq.is_ok(), Error::<T, I>::BalanceLow);
					let actual = actual.saturating_add(conseq.unwrap());

					account.reserved_balance -= actual;

					// defensive only: this can never fail since total issuance which is at least
					// free+reserved fits into the same data type.
					account.balance = account.balance.defensive_saturating_add(actual);

					Ok(actual)
				},
			}
		}) {
			Ok(x) => x,
			Err(err) => return Err(err.into()),
		};

		Ok(amount - actual)
	}

	/// Transfer held funds into a destination account.
	///
	/// If `on_hold` is `true`, then the destination account must already exist and the assets
	/// transferred will still be on hold in the destination account. If not, then the destination
	/// account need not already exist, but must be creatable.
	///
	/// If `best_effort` is `true`, then an amount less than `amount` may be transferred without
	/// error.
	///
	/// The actual amount transferred is returned, or `Err` in the case of error and nothing is
	/// changed.
	fn transfer_held(
		asset: Self::AssetId,
		source: &T::AccountId,
		dest: &T::AccountId,
		amount: Self::Balance,
		best_effort: bool,
		on_hold: bool,
	) -> Result<Self::Balance, DispatchError> {
		if amount.is_zero() {
			return Ok(Zero::zero())
		}

		if Self::balance_on_hold(asset, source).is_zero() {
			return Ok(amount)
		}

		Asset::<T, I>::try_mutate(asset, |maybe_details| -> Result<Self::Balance, DispatchError> {
			let details = maybe_details.as_mut().ok_or(Error::<T, I>::Unknown)?;

			Account::<T, I>::try_mutate(
				asset,
				source,
				|maybe_details| -> Result<T::Balance, DispatchError> {
					ensure!(maybe_details.is_some(), Error::<T, I>::NoAccount);

					match maybe_details {
						None => Ok(Zero::zero()),
						Some(source) => {
							let actual = cmp::min(source.reserved_balance, amount);
							ensure!(best_effort || actual == amount, Error::<T, I>::BalanceLow);

							source.reserved_balance -=
								match Account::<T, I>::try_mutate(asset, dest, |maybe_details| {
									match maybe_details {
										maybe_account @ None => {
											if on_hold {
												return Err(Error::<T, I>::NoAccount.into())
											}

											*maybe_account = Some(AssetAccountOf::<T, I> {
												balance: Zero::zero(),
												reserved_balance: actual,
												frozen_balance: Zero::zero(),
												is_frozen: false,
												reason: Self::new_account(dest, details, None)?,
												extra: T::Extra::default(),
											});

											Ok(actual)
										},
										Some(dest) => {
											dest.reserved_balance = dest
												.reserved_balance
												.checked_add(&actual)
												.ok_or(ArithmeticError::Overflow)?;

											Ok(actual)
										},
									}
								}) {
									Ok(x) => x,
									Err(err) => return Err(err),
								};

							Ok(actual)
						},
					}
				},
			)
		})
	}
}
