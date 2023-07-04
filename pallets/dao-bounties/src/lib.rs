//! # DAO Bounties Module ( pallet-dao-bounties )
//!
//! ## Bounty
//!
//! > NOTE: This pallet is tightly coupled with pallet-dao-treasury.
//!
//! A Bounty Spending is a reward for a specified body of work - or specified set of objectives -
//! that needs to be executed for a predefined Treasury amount to be paid out. A curator is assigned
//! after the bounty is approved and funded by Council, to be delegated with the responsibility of
//! assigning a payout address once the specified set of objectives is completed.
//!
//! After the Council has activated a bounty, it delegates the work that requires expertise to a
//! curator in exchange of a deposit. Once the curator accepts the bounty, they get to close the
//! active bounty. Closing the active bounty enacts a delayed payout to the payout address, the
//! curator fee and the return of the curator deposit. The delay allows for intervention through
//! regular democracy. The Council gets to unassign the curator, resulting in a new curator
//! election. The Council also gets to cancel the bounty if deemed necessary before assigning a
//! curator or once the bounty is active or payout is pending, resulting in the slash of the
//! curator's deposit.
//!
//! This pallet may opt into using a [`ChildBountyManager`] that enables bounties to be split into
//! sub-bounties, as children of anh established bounty (called the parent in the context of it's
//! children).
//!
//! > NOTE: The parent bounty cannot be closed if it has a non-zero number of it has active child
//! > bounties associated with it.
//!
//! ### Terminology
//!
//! Bounty:
//!
//! - **Bounty spending proposal:** A proposal to reward a predefined body of work upon completion
//!   by the Treasury.
//! - **Proposer:** An account proposing a bounty spending.
//! - **Curator:** An account managing the bounty and assigning a payout address receiving the
//!   reward for the completion of work.
//! - **Deposit:** The amount held on deposit for placing a bounty proposal plus the amount held on
//!   deposit per byte within the bounty description.
//! - **Curator deposit:** The payment from a candidate willing to curate an approved bounty. The
//!   deposit is returned when/if the bounty is completed.
//! - **Bounty value:** The total amount that should be paid to the Payout Address if the bounty is
//!   rewarded.
//! - **Payout address:** The account to which the total or part of the bounty is assigned to.
//! - **Payout Delay:** The delay period for which a bounty beneficiary needs to wait before
//!   claiming.
//! - **Curator fee:** The reserved upfront payment for a curator for work related to the bounty.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! Bounty protocol:
//!
//! - `create_bounty` - Accept a specific treasury amount to be earmarked for a predefined body of
//!   work.
//! - `propose_curator` - Assign an account to a bounty as candidate curator.
//! - `accept_curator` - Accept a bounty assignment from the Council, setting a curator deposit.
//! - `extend_bounty_expiry` - Extend the expiry block number of the bounty and stay active.
//! - `award_bounty` - Close and pay out the specified amount for the completed work.
//! - `claim_bounty` - Claim a specific bounty amount from the Payout Address.
//! - `unassign_curator` - Unassign an accepted curator from a specific earmark.
//! - `close_bounty` - Cancel the earmark for a specific treasury amount and close the bounty.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod tests;
pub mod weights;

use sp_std::prelude::*;

use pallet_dao_assets::LockableAsset;

use frame_support::traits::{
	fungibles::{BalancedHold, Inspect, InspectHold, MutateHold},
	Currency,
	ExistenceRequirement::AllowDeath,
	Get, Imbalance, ReservableCurrency,
};

use sp_runtime::{
	traits::{AccountIdConversion, BadOrigin, Saturating, Zero},
	DispatchResult, RuntimeDebug,
};

use frame_support::dispatch::DispatchResultWithPostInfo;

use dao_primitives::{DaoGovernance, DaoPolicy, DaoProvider, DaoToken};
use frame_support::{pallet_prelude::*, traits::fungibles::Transfer};
use frame_system::pallet_prelude::*;
use scale_info::TypeInfo;
pub use weights::WeightInfo;

pub use pallet::*;
use pallet_dao_treasury::DaoId;

type BalanceOf<T, I = ()> = pallet_dao_treasury::BalanceOf<T, I>;

type PositiveImbalanceOf<T, I = ()> = pallet_dao_treasury::PositiveImbalanceOf<T, I>;

/// An index of a bounty. Just a `u32`.
pub type BountyIndex = u32;

/// A bounty proposal.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct Bounty<AccountId, Balance, BlockNumber, TokenId> {
	/// Token ID if bounty is assets-based
	token_id: Option<TokenId>,
	/// The (total) amount that should be paid if the bounty is rewarded.
	value: Balance,
	/// The curator fee. Included in value.
	fee: Balance,
	/// The deposit of curator.
	curator_deposit: Balance,
	/// The status of this bounty.
	status: BountyStatus<AccountId, BlockNumber>,
}

impl<AccountId: PartialEq + Clone + Ord, Balance, BlockNumber: Clone, TokenId>
	Bounty<AccountId, Balance, BlockNumber, TokenId>
{
	/// Getter for bounty status, to be used for child bounties.
	pub fn get_status(&self) -> BountyStatus<AccountId, BlockNumber> {
		self.status.clone()
	}
}

/// The status of a bounty proposal.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum BountyStatus<AccountId, BlockNumber> {
	/// The bounty is approved and waiting to become active at next spend period.
	Approved,
	/// The bounty is funded and waiting for curator assignment.
	Funded,
	/// A curator has been proposed by the `ApproveOrigin`. Waiting for acceptance from the
	/// curator.
	CuratorProposed {
		/// The assigned curator of this bounty.
		curator: AccountId,
	},
	/// The bounty is active and waiting to be awarded.
	Active {
		/// The curator of this bounty.
		curator: AccountId,
		/// An update from the curator is due by this block, else they are considered inactive.
		update_due: BlockNumber,
	},
	/// The bounty is awarded and waiting to released after a delay.
	PendingPayout {
		/// The curator of this bounty.
		curator: AccountId,
		/// The beneficiary of the bounty.
		beneficiary: AccountId,
		/// When the bounty can be claimed.
		unlock_at: BlockNumber,
	},
}

/// The child bounty manager.
pub trait ChildBountyManager<Balance> {
	/// Get the active child bounties for a parent bounty.
	fn child_bounties_count(bounty_id: BountyIndex) -> BountyIndex;

	/// Get total curator fees of children-bounty curators.
	fn children_curator_fees(bounty_id: BountyIndex) -> Balance;
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::traits::{OnUnbalanced, TryDrop};

	#[pallet::pallet]
	pub struct Pallet<T, I = ()>(_);

	#[pallet::config]
	pub trait Config<I: 'static = ()>:
		frame_system::Config + pallet_dao_treasury::Config<I>
	{
		type Assets: LockableAsset<
				Self::AccountId,
				AssetId = Self::AssetId,
				Moment = Self::BlockNumber,
				Balance = BalanceOf<Self, I>,
			> + Inspect<Self::AccountId>
			+ InspectHold<Self::AccountId>
			+ MutateHold<Self::AccountId>
			+ BalancedHold<Self::AccountId>;

		/// The overarching event type.
		type RuntimeEvent: From<Event<Self, I>>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Maximum acceptable reason length.
		///
		/// Benchmarks depend on this value, be sure to update weights file when changing this value
		#[pallet::constant]
		type MaximumReasonLength: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;

		/// The child bounty manager.
		type ChildBountyManager: ChildBountyManager<BalanceOf<Self, I>>;
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// Proposer's balance is too low.
		InsufficientProposersBalance,
		/// No proposal or bounty at that index.
		InvalidIndex,
		/// The reason given is just too big.
		ReasonTooBig,
		/// The bounty status is unexpected.
		UnexpectedStatus,
		/// Require bounty curator.
		RequireCurator,
		/// Invalid bounty value.
		InvalidValue,
		/// Invalid bounty fee.
		InvalidFee,
		/// A bounty payout is pending.
		/// To cancel the bounty, you must unassign and slash the curator.
		PendingPayout,
		/// The bounties cannot be claimed/closed because it's still in the countdown period.
		Premature,
		/// The bounty cannot be closed because it has active child bounties.
		HasActiveChildBounty,
		/// Too many approvals are already queued.
		TooManyQueued,
		/// Insufficient balance
		InsufficientBalance,
		NotSupported,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		/// New bounty proposal.
		BountyCreated {
			dao_id: DaoId,
			index: BountyIndex,
			status: BountyStatus<T::AccountId, T::BlockNumber>,
			description: Vec<u8>,
			value: BalanceOf<T, I>,
			token_id: Option<T::AssetId>,
		},
		/// A bounty proposal is funded and became active.
		BountyBecameActive {
			dao_id: DaoId,
			index: BountyIndex,
			status: BountyStatus<T::AccountId, T::BlockNumber>,
		},
		/// A curator is proposed for bounty.
		BountyCuratorProposed {
			dao_id: DaoId,
			index: BountyIndex,
			fee: BalanceOf<T, I>,
			status: BountyStatus<T::AccountId, T::BlockNumber>,
		},
		/// A curator is unassigned from bounty.
		BountyCuratorUnassigned {
			dao_id: DaoId,
			index: BountyIndex,
			status: BountyStatus<T::AccountId, T::BlockNumber>,
		},
		/// A curator is accepted for bounty.
		BountyCuratorAccepted {
			dao_id: DaoId,
			index: BountyIndex,
			status: BountyStatus<T::AccountId, T::BlockNumber>,
		},
		/// A bounty is awarded to a beneficiary.
		BountyAwarded {
			dao_id: DaoId,
			index: BountyIndex,
			beneficiary: T::AccountId,
			status: BountyStatus<T::AccountId, T::BlockNumber>,
		},
		/// A bounty is claimed by beneficiary.
		BountyClaimed {
			dao_id: DaoId,
			index: BountyIndex,
			payout: BalanceOf<T, I>,
			beneficiary: T::AccountId,
		},
		/// A bounty is cancelled.
		BountyCanceled { dao_id: DaoId, index: BountyIndex },
		/// A bounty expiry is extended.
		BountyExtended { dao_id: DaoId, index: BountyIndex, update_due: T::BlockNumber },
	}

	/// Number of bounty proposals that have been made.
	#[pallet::storage]
	#[pallet::getter(fn bounty_count)]
	pub type BountyCount<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Twox64Concat, DaoId, BountyIndex, ValueQuery>;

	/// Bounties that have been made.
	#[pallet::storage]
	#[pallet::getter(fn bounties)]
	pub type Bounties<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
		_,
		Twox64Concat,
		DaoId,
		Twox64Concat,
		BountyIndex,
		Bounty<T::AccountId, BalanceOf<T, I>, T::BlockNumber, T::AssetId>,
	>;

	/// The description of each bounty.
	#[pallet::storage]
	#[pallet::getter(fn bounty_descriptions)]
	pub type BountyDescriptions<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
		_,
		Twox64Concat,
		DaoId,
		Twox64Concat,
		BountyIndex,
		BoundedVec<u8, T::MaximumReasonLength>,
	>;

	/// Bounty indices that have been approved but not yet funded.
	#[pallet::storage]
	#[pallet::getter(fn bounty_approvals)]
	pub type BountyApprovals<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Twox64Concat, DaoId, BoundedVec<BountyIndex, T::MaxApprovals>, ValueQuery>;

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		/// Create new bounty.
		///
		/// May only be called from `T::ApproveOrigin`.
		///
		/// # <weight>
		/// - O(1).
		/// # </weight>
		#[pallet::weight(<T as Config<I>>::WeightInfo::create_bounty())]
		#[pallet::call_index(0)]
		pub fn create_bounty(
			origin: OriginFor<T>,
			dao_id: DaoId,
			#[pallet::compact] value: BalanceOf<T, I>,
			description: Vec<u8>,
		) -> DispatchResult {
			Self::create_token_bounty(origin, dao_id, None, value, description)
		}

		/// Create new bounty. At a later time, the bounty will be funded.
		///
		/// May only be called from `T::ApproveOrigin`.
		///
		/// # <weight>
		/// - O(1).
		/// # </weight>
		#[pallet::weight(<T as Config<I>>::WeightInfo::create_bounty())]
		#[pallet::call_index(1)]
		pub fn create_token_bounty(
			origin: OriginFor<T>,
			dao_id: DaoId,
			token_id: Option<T::AssetId>,
			#[pallet::compact] value: BalanceOf<T, I>,
			description: Vec<u8>,
		) -> DispatchResult {
			T::DaoProvider::ensure_approved(origin, dao_id)?;

			Self::do_create_bounty(dao_id, token_id, description, value)?;

			Ok(())
		}

		/// Assign a curator to a funded bounty.
		///
		/// May only be called from `T::ApproveOrigin`.
		///
		/// # <weight>
		/// - O(1).
		/// # </weight>
		#[pallet::weight(<T as Config<I>>::WeightInfo::propose_curator())]
		#[pallet::call_index(2)]
		pub fn propose_curator(
			origin: OriginFor<T>,
			dao_id: DaoId,
			#[pallet::compact] bounty_id: BountyIndex,
			curator: T::AccountId,
			#[pallet::compact] fee: BalanceOf<T, I>,
		) -> DispatchResult {
			T::DaoProvider::ensure_approved(origin, dao_id)?;

			Bounties::<T, I>::try_mutate_exists(
				dao_id,
				bounty_id,
				|maybe_bounty| -> DispatchResult {
					let mut bounty = maybe_bounty.as_mut().ok_or(Error::<T, I>::InvalidIndex)?;
					match bounty.status {
						BountyStatus::Funded => {},
						_ => return Err(Error::<T, I>::UnexpectedStatus.into()),
					};

					ensure!(fee < bounty.value, Error::<T, I>::InvalidFee);

					bounty.status = BountyStatus::CuratorProposed { curator: curator.clone() };
					bounty.fee = fee;

					Ok(())
				},
			)?;

			Self::deposit_event(Event::<T, I>::BountyCuratorProposed {
				dao_id,
				index: bounty_id,
				fee,
				status: BountyStatus::CuratorProposed { curator },
			});

			Ok(())
		}

		/// Unassign curator from a bounty.
		///
		/// This function can only be called by the `ApproveOrigin` a signed origin.
		///
		/// If this function is called by the `ApproveOrigin`, we assume that the curator is
		/// malicious or inactive. As a result, we will slash the curator when possible.
		///
		/// If the origin is the curator, we take this as a sign they are unable to do their job and
		/// they willingly give up. We could slash them, but for now we allow them to recover their
		/// deposit and exit without issue. (We may want to change this if it is abused.)
		///
		/// Finally, the origin can be anyone if and only if the curator is "inactive". This allows
		/// anyone in the community to call out that a curator is not doing their due diligence, and
		/// we should pick a new curator. In this case the curator should also be slashed.
		///
		/// # <weight>
		/// - O(1).
		/// # </weight>
		#[pallet::weight((<T as Config<I>>::WeightInfo::unassign_curator(), DispatchClass::Normal, Pays::No))]
		#[pallet::call_index(3)]
		pub fn unassign_curator(
			origin: OriginFor<T>,
			dao_id: DaoId,
			#[pallet::compact] bounty_id: BountyIndex,
		) -> DispatchResultWithPostInfo {
			let maybe_sender = ensure_signed(origin.clone())
				.map(Some)
				.or_else(|_| T::DaoProvider::ensure_approved(origin, dao_id).map(|_| None))?;

			// TODO: check dao origin

			Bounties::<T, I>::try_mutate_exists(
				dao_id,
				bounty_id,
				|maybe_bounty| -> DispatchResult {
					let mut bounty = maybe_bounty.as_mut().ok_or(Error::<T, I>::InvalidIndex)?;

					let slash_curator =
						|curator: &T::AccountId, curator_deposit: &mut BalanceOf<T, I>| {
							match bounty.token_id {
								None => {
									let imbalance =
										T::Currency::slash_reserved(curator, *curator_deposit).0;
									T::OnSlash::on_unbalanced(imbalance);
								},
								Some(token_id) => {
									let (credit_of, _balance) =
										T::Assets::slash_held(token_id, curator, *curator_deposit);
									// TODO: check
									if let Ok(()) = credit_of.try_drop() {}
								},
							}

							*curator_deposit = Zero::zero();
						};

					match bounty.status {
						BountyStatus::Approved | BountyStatus::Funded => {
							// No curator to unassign at this point.
							return Err(Error::<T, I>::UnexpectedStatus.into())
						},
						BountyStatus::CuratorProposed { ref curator } => {
							// A curator has been proposed, but not accepted yet.
							// Either `ApproveOrigin` or the proposed curator can unassign the
							// curator.
							ensure!(
								maybe_sender.map_or(true, |sender| sender == *curator),
								BadOrigin
							);
						},
						BountyStatus::Active { ref curator, ref update_due } => {
							// The bounty is active.
							match maybe_sender {
								// If the `ApproveOrigin` is calling this function, slash the
								// curator.
								None => {
									slash_curator(curator, &mut bounty.curator_deposit);
									// Continue to change bounty status below...
								},
								Some(sender) => {
									// If the sender is not the curator, and the curator is
									// inactive, slash the curator.
									if sender != *curator {
										let block_number =
											frame_system::Pallet::<T>::block_number();
										if *update_due < block_number {
											slash_curator(curator, &mut bounty.curator_deposit);
										// Continue to change bounty status below...
										} else {
											// Curator has more time to give an update.
											return Err(Error::<T, I>::Premature.into())
										}
									} else {
										// Else this is the curator, willingly giving up their role.
										// Give back their deposit.
										let err_amount = match bounty.token_id {
											None => T::Currency::unreserve(
												curator,
												bounty.curator_deposit,
											),
											Some(token_id) => T::Assets::release(
												token_id,
												curator,
												bounty.curator_deposit,
												true,
											)?,
										};
										debug_assert!(err_amount.is_zero());
										bounty.curator_deposit = Zero::zero();
										// Continue to change bounty status below...
									}
								},
							}
						},
						BountyStatus::PendingPayout { ref curator, .. } => {
							// The bounty is pending payout, so only council can unassign a curator.
							// By doing so, they are claiming the curator is acting maliciously, so
							// we slash the curator.
							ensure!(maybe_sender.is_none(), BadOrigin);
							slash_curator(curator, &mut bounty.curator_deposit);
							// Continue to change bounty status below...
						},
					};

					bounty.status = BountyStatus::Funded;
					Ok(())
				},
			)?;

			Self::deposit_event(Event::<T, I>::BountyCuratorUnassigned {
				dao_id,
				index: bounty_id,
				status: BountyStatus::Funded,
			});

			Ok((Some(<T as Config<I>>::WeightInfo::unassign_curator()), Pays::No).into())
		}

		/// Accept the curator role for a bounty.
		/// A deposit will be reserved from curator and refund upon successful payout.
		///
		/// May only be called from the curator.
		///
		/// # <weight>
		/// - O(1).
		/// # </weight>
		#[pallet::weight((<T as Config<I>>::WeightInfo::accept_curator(), DispatchClass::Normal, Pays::No))]
		#[pallet::call_index(4)]
		pub fn accept_curator(
			origin: OriginFor<T>,
			dao_id: DaoId,
			#[pallet::compact] bounty_id: BountyIndex,
		) -> DispatchResultWithPostInfo {
			let signer = ensure_signed(origin)?;

			let dao_policy = T::DaoProvider::policy(dao_id)?;

			let mut status: Option<BountyStatus<T::AccountId, T::BlockNumber>> = None;

			Bounties::<T, I>::try_mutate_exists(
				dao_id,
				bounty_id,
				|maybe_bounty| -> DispatchResult {
					let mut bounty = maybe_bounty.as_mut().ok_or(Error::<T, I>::InvalidIndex)?;

					match bounty.status {
						BountyStatus::CuratorProposed { ref curator } => {
							ensure!(signer == *curator, Error::<T, I>::RequireCurator);

							if bounty.fee > Zero::zero() {
								match bounty.token_id {
									None => T::Currency::reserve(curator, bounty.fee)?,
									Some(token_id) =>
										T::Assets::hold(token_id, curator, bounty.fee)?,
								}
							}

							bounty.curator_deposit = bounty.fee;

							let update_due = frame_system::Pallet::<T>::block_number() +
								dao_policy.bounty_update_period.0.into();

							let active_status =
								BountyStatus::Active { curator: curator.clone(), update_due };

							bounty.status = active_status.clone();

							status = Some(active_status);

							Ok(())
						},
						_ => Err(Error::<T, I>::UnexpectedStatus.into()),
					}
				},
			)?;

			Self::deposit_event(Event::<T, I>::BountyCuratorAccepted {
				dao_id,
				index: bounty_id,
				status: status.unwrap(),
			});

			Ok((Some(<T as Config<I>>::WeightInfo::accept_curator()), Pays::No).into())
		}

		/// Award bounty to a beneficiary account. The beneficiary will be able to claim the funds
		/// after a delay.
		///
		/// The dispatch origin for this call must be the curator of this bounty.
		///
		/// - `bounty_id`: Bounty ID to award.
		/// - `beneficiary`: The beneficiary account whom will receive the payout.
		///
		/// # <weight>
		/// - O(1).
		/// # </weight>
		#[pallet::weight((
			<T as Config<I>>::WeightInfo::award_bounty(),
			DispatchClass::Normal,
			Pays::No
		))]
		#[pallet::call_index(5)]
		pub fn award_bounty(
			origin: OriginFor<T>,
			dao_id: DaoId,
			#[pallet::compact] bounty_id: BountyIndex,
			beneficiary: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let signer = ensure_signed(origin)?;

			let dao_policy = T::DaoProvider::policy(dao_id)?;

			let mut status: Option<BountyStatus<T::AccountId, T::BlockNumber>> = None;

			Bounties::<T, I>::try_mutate_exists(
				dao_id,
				bounty_id,
				|maybe_bounty| -> DispatchResult {
					let mut bounty = maybe_bounty.as_mut().ok_or(Error::<T, I>::InvalidIndex)?;

					// Ensure no active child bounties before processing the call.
					ensure!(
						T::ChildBountyManager::child_bounties_count(bounty_id) == 0,
						Error::<T, I>::HasActiveChildBounty
					);

					match &bounty.status {
						BountyStatus::Active { curator, .. } => {
							ensure!(signer == *curator, Error::<T, I>::RequireCurator);
						},
						_ => return Err(Error::<T, I>::UnexpectedStatus.into()),
					}

					let pending_payout_status = BountyStatus::PendingPayout {
						curator: signer,
						beneficiary: beneficiary.clone(),
						unlock_at: frame_system::Pallet::<T>::block_number() +
							dao_policy.bounty_payout_delay.0.into(),
					};

					bounty.status = pending_payout_status.clone();

					status = Some(pending_payout_status);

					Ok(())
				},
			)?;

			Self::deposit_event(Event::<T, I>::BountyAwarded {
				dao_id,
				index: bounty_id,
				status: status.unwrap(),
				beneficiary,
			});

			Ok((Some(<T as Config<I>>::WeightInfo::award_bounty()), Pays::No).into())
		}

		/// Claim the payout from an awarded bounty after payout delay.
		///
		/// The dispatch origin for this call must be the beneficiary of this bounty.
		///
		/// - `bounty_id`: Bounty ID to claim.
		///
		/// # <weight>
		/// - O(1).
		/// # </weight>
		#[pallet::weight((
			<T as Config<I>>::WeightInfo::claim_bounty(),
			DispatchClass::Normal,
			Pays::No
		))]
		#[pallet::call_index(6)]
		pub fn claim_bounty(
			origin: OriginFor<T>,
			dao_id: DaoId,
			#[pallet::compact] bounty_id: BountyIndex,
		) -> DispatchResultWithPostInfo {
			let _ = ensure_signed(origin)?; // anyone can trigger claim

			Bounties::<T, I>::try_mutate_exists(
				dao_id,
				bounty_id,
				|maybe_bounty| -> DispatchResult {
					let bounty = maybe_bounty.take().ok_or(Error::<T, I>::InvalidIndex)?;
					if let BountyStatus::PendingPayout { curator, beneficiary, unlock_at } =
						bounty.status
					{
						ensure!(
							frame_system::Pallet::<T>::block_number() >= unlock_at,
							Error::<T, I>::Premature
						);
						let bounty_account = Self::bounty_account_id(dao_id, bounty_id);
						let balance = T::Currency::free_balance(&bounty_account);
						let fee = bounty.fee.min(balance); // just to be safe
						let payout = balance.saturating_sub(fee);
						let err_amount = T::Currency::unreserve(&curator, bounty.curator_deposit);
						debug_assert!(err_amount.is_zero());

						// Get total child bounties curator fees, and subtract it from the parent
						// curator fee (the fee in present referenced bounty, `self`).
						let children_fee = T::ChildBountyManager::children_curator_fees(bounty_id);
						debug_assert!(children_fee <= fee);

						let final_fee = fee.saturating_sub(children_fee);
						let res =
							T::Currency::transfer(&bounty_account, &curator, final_fee, AllowDeath); // should not fail
						debug_assert!(res.is_ok());
						let res = T::Currency::transfer(
							&bounty_account,
							&beneficiary,
							payout,
							AllowDeath,
						); // should not fail
						debug_assert!(res.is_ok());

						*maybe_bounty = None;

						BountyDescriptions::<T, I>::remove(dao_id, bounty_id);

						Self::deposit_event(Event::<T, I>::BountyClaimed {
							dao_id,
							index: bounty_id,
							payout,
							beneficiary,
						});
						Ok(())
					} else {
						Err(Error::<T, I>::UnexpectedStatus.into())
					}
				},
			)?;

			Ok((Some(<T as Config<I>>::WeightInfo::claim_bounty()), Pays::No).into())
		}

		/// Cancel a proposed or active bounty. All the funds will be sent to treasury and
		/// the curator deposit will be unreserved if possible.
		///
		/// Only `T::ApproveOrigin` is able to cancel a bounty.
		///
		/// - `bounty_id`: Bounty ID to cancel.
		///
		/// # <weight>
		/// - O(1).
		/// # </weight>
		#[pallet::weight(<T as Config<I>>::WeightInfo::close_bounty_active())]
		#[pallet::call_index(7)]
		pub fn close_bounty(
			origin: OriginFor<T>,
			dao_id: DaoId,
			#[pallet::compact] bounty_id: BountyIndex,
		) -> DispatchResultWithPostInfo {
			T::DaoProvider::ensure_approved(origin, dao_id)?;

			Bounties::<T, I>::try_mutate_exists(
				dao_id,
				bounty_id,
				|maybe_bounty| -> DispatchResultWithPostInfo {
					let bounty = maybe_bounty.as_ref().ok_or(Error::<T, I>::InvalidIndex)?;

					// Ensure no active child bounties before processing the call.
					ensure!(
						T::ChildBountyManager::child_bounties_count(bounty_id) == 0,
						Error::<T, I>::HasActiveChildBounty
					);

					match &bounty.status {
						BountyStatus::Approved => {
							// For weight reasons, we don't allow a council to cancel in this phase.
							// We ask for them to wait until it is funded before they can cancel.
							return Err(Error::<T, I>::UnexpectedStatus.into())
						},
						BountyStatus::Funded | BountyStatus::CuratorProposed { .. } => {
							// Nothing extra to do besides the removal of the bounty below.
						},
						BountyStatus::Active { curator, .. } => {
							// Cancelled by council, refund deposit of the working curator.
							let err_amount =
								T::Currency::unreserve(curator, bounty.curator_deposit);
							debug_assert!(err_amount.is_zero());
							// Then execute removal of the bounty below.
						},
						BountyStatus::PendingPayout { .. } => {
							// Bounty is already pending payout. If council wants to cancel
							// this bounty, it should mean the curator was acting maliciously.
							// So the council should first unassign the curator, slashing their
							// deposit.
							return Err(Error::<T, I>::PendingPayout.into())
						},
					}

					let bounty_account = Self::bounty_account_id(dao_id, bounty_id);

					BountyDescriptions::<T, I>::remove(dao_id, bounty_id);

					match bounty.token_id {
						None => {
							let balance = T::Currency::free_balance(&bounty_account);
							T::Currency::transfer(
								&bounty_account,
								&T::DaoProvider::dao_account_id(dao_id),
								balance,
								AllowDeath,
							)?;
						},
						Some(token_id) => {
							let balance = T::Assets::balance(token_id, &bounty_account);
							T::Assets::transfer(
								token_id,
								&bounty_account,
								&T::DaoProvider::dao_account_id(dao_id),
								balance,
								false,
							)?;
						},
					};

					*maybe_bounty = None;

					Self::deposit_event(Event::<T, I>::BountyCanceled { dao_id, index: bounty_id });
					Ok(Some(<T as Config<I>>::WeightInfo::close_bounty_active()).into())
				},
			)
		}

		/// Extend the expiry time of an active bounty.
		///
		/// The dispatch origin for this call must be the curator of this bounty.
		///
		/// - `bounty_id`: Bounty ID to extend.
		/// - `remark`: additional information.
		///
		/// # <weight>
		/// - O(1).
		/// # </weight>
		#[pallet::weight((
			<T as Config<I>>::WeightInfo::extend_bounty_expiry(),
			DispatchClass::Normal,
			Pays::No
		))]
		#[pallet::call_index(8)]
		pub fn extend_bounty_expiry(
			origin: OriginFor<T>,
			dao_id: DaoId,
			#[pallet::compact] bounty_id: BountyIndex,
			_remark: Vec<u8>,
		) -> DispatchResultWithPostInfo {
			let signer = ensure_signed(origin)?;

			let dao_policy = T::DaoProvider::policy(dao_id)?;

			let mut new_update_due = None;

			Bounties::<T, I>::try_mutate_exists(
				dao_id,
				bounty_id,
				|maybe_bounty| -> DispatchResult {
					let bounty = maybe_bounty.as_mut().ok_or(Error::<T, I>::InvalidIndex)?;

					match bounty.status {
						BountyStatus::Active { ref curator, ref mut update_due } => {
							ensure!(*curator == signer, Error::<T, I>::RequireCurator);
							*update_due = (frame_system::Pallet::<T>::block_number() +
								dao_policy.bounty_update_period.0.into())
							.max(*update_due);

							new_update_due = Some(*update_due);
						},
						_ => return Err(Error::<T, I>::UnexpectedStatus.into()),
					}

					Ok(())
				},
			)?;

			Self::deposit_event(Event::<T, I>::BountyExtended {
				dao_id,
				index: bounty_id,
				update_due: new_update_due.unwrap(),
			});

			Ok((Some(<T as Config<I>>::WeightInfo::extend_bounty_expiry()), Pays::No).into())
		}
	}
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
	// TODO: check bounty account id uniqueness
	/// The account ID of a bounty account
	pub fn bounty_account_id(dao_id: DaoId, id: BountyIndex) -> T::AccountId {
		// only use two byte prefix to support 16 byte account id (used by test)
		// "modl" ++ "py/dtrsr" ++ "bt" is 14 bytes, and two bytes remaining for bounty index
		T::PalletId::get().into_sub_account_truncating(("bt", dao_id, id))
	}

	fn do_create_bounty(
		dao_id: DaoId,
		token_id: Option<T::AssetId>,
		description: Vec<u8>,
		value: BalanceOf<T, I>,
	) -> DispatchResult {
		let bounded_description: BoundedVec<_, _> =
			description.clone().try_into().map_err(|_| Error::<T, I>::ReasonTooBig)?;
		ensure!(value != Zero::zero(), Error::<T, I>::InvalidValue);

		let DaoPolicy { governance, .. } = T::DaoProvider::policy(dao_id)?;
		if token_id.is_some() && Some(DaoGovernance::OwnershipWeightedVoting) == governance {
			return Err(Error::<T, I>::NotSupported.into())
		}

		let dao_account_id = T::DaoProvider::dao_account_id(dao_id);
		let balance = match token_id {
			None => T::Currency::free_balance(&dao_account_id),
			Some(token_id) => T::Assets::balance(token_id, &dao_account_id),
		};
		ensure!(balance >= value, Error::<T, I>::InsufficientBalance);

		let index = Self::bounty_count(dao_id);
		BountyCount::<T, I>::insert(dao_id, index + 1);

		let bounty = Bounty {
			token_id,
			value,
			fee: 0u32.into(),
			curator_deposit: 0u32.into(),
			status: BountyStatus::Approved,
		};

		BountyApprovals::<T, I>::try_append(dao_id, index)
			.map_err(|()| Error::<T, I>::TooManyQueued)?;

		Bounties::<T, I>::insert(dao_id, index, &bounty);
		BountyDescriptions::<T, I>::insert(dao_id, index, bounded_description);

		Self::deposit_event(Event::<T, I>::BountyCreated {
			dao_id,
			index,
			status: bounty.status,
			description,
			value,
			token_id,
		});

		Ok(())
	}

	pub fn dao_token_id(dao_id: DaoId) -> Result<T::AssetId, DispatchError> {
		match T::DaoProvider::dao_token(dao_id)? {
			DaoToken::FungibleToken(token_id) => Ok(token_id),
			DaoToken::EthTokenAddress(_) => Err(Error::<T, I>::NotSupported.into()),
		}
	}
}

impl<T: Config<I>, I: 'static> pallet_dao_treasury::SpendFunds<T, I> for Pallet<T, I> {
	fn spend_funds(
		dao_id: DaoId,
		budget_remaining: &mut BalanceOf<T, I>,
		imbalance: &mut PositiveImbalanceOf<T, I>,
		total_weight: &mut Weight,
		missed_any: &mut bool,
	) {
		let dao_account_id = T::DaoProvider::dao_account_id(dao_id);

		let bounties_len = BountyApprovals::<T, I>::mutate(dao_id, |v| {
			let bounties_approval_len = v.len() as u32;
			v.retain(|&index| {
				Bounties::<T, I>::mutate(dao_id, index, |bounty| {
					// Should always be true, but shouldn't panic if false or we're screwed.
					if let Some(bounty) = bounty {
						match bounty.token_id {
							None => {
								if bounty.value <= *budget_remaining {
									*budget_remaining -= bounty.value;

									bounty.status = BountyStatus::Funded;

									// fund the bounty account
									imbalance.subsume(T::Currency::deposit_creating(
										&Self::bounty_account_id(dao_id, index),
										bounty.value,
									));

									Self::deposit_event(Event::<T, I>::BountyBecameActive {
										dao_id,
										index,
										status: bounty.clone().status,
									});
									false
								} else {
									*missed_any = true;
									true
								}
							},
							Some(token_id) => {
								let budget_remaining = T::Assets::balance(
									token_id,
									&T::DaoProvider::dao_account_id(dao_id),
								);

								if bounty.value <= budget_remaining {
									match T::Assets::transfer(
										token_id,
										&dao_account_id,
										&Self::bounty_account_id(dao_id, index),
										bounty.value,
										true,
									) {
										Ok(_) => {},
										Err(_) => {
											*missed_any = true;

											return false
										},
									};

									Self::deposit_event(Event::<T, I>::BountyBecameActive {
										dao_id,
										index,
										status: bounty.clone().status,
									});
									false
								} else {
									*missed_any = true;
									true
								}
							},
						}
					} else {
						false
					}
				})
			});
			bounties_approval_len
		});

		*total_weight += <T as pallet::Config<I>>::WeightInfo::spend_funds(bounties_len);
	}
}

// Default impl for when ChildBounties is not being used in the runtime.
impl<Balance: Zero> ChildBountyManager<Balance> for () {
	fn child_bounties_count(_bounty_id: BountyIndex) -> BountyIndex {
		Default::default()
	}

	fn children_curator_fees(_bounty_id: BountyIndex) -> Balance {
		Zero::zero()
	}
}
