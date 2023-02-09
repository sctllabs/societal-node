// Re-purposing the original treasury pallet to manage funds for the DAOs created by the pallet-dao
// factory:
// - re-worked pallet storage to persist proposals/approvals for each DAO
// - updated pallet extrinsic functions adding dao support
// - added support for DaoProvider retrieving custom configuration for each DAO
// - updated origins using EnsureOriginWithArg to support custom configuration by DaoProvider
// - removed GenesisConfig

//! # DAO Treasury Pallet
//!
//! The DAO Treasury pallet provides a "pot" of funds that can be managed by DAO members.
//!
//! - [`Config`]
//! - [`Call`]
//!
//! ## Overview
//!
//! The DAO Treasury Pallet itself provides the pot to store and means for DAO members to
//! manage expenditures via external origins.
//!
//!
//! ### Terminology
//!
//! - **Beneficiary:** An account who will receive the funds from a proposal iff the proposal is
//!   approved.
//! - **Pot:** Unspent funds accumulated by the treasury pallet.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! General spending/proposal protocol:
//! - `spend` - Spend DAO native tokens.
//! - `transfer_token` - Transfer DAO Governance Token.
//! - `transfer_token_by_id` - Transfer any token owned by the DAO.

#![cfg_attr(not(feature = "std"), no_std)]

mod benchmarking;
#[cfg(test)]
mod tests;
pub mod weights;

use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

use sp_runtime::{
	traits::{AccountIdConversion, Saturating, StaticLookup, Zero},
	Permill, RuntimeDebug,
};
use sp_std::prelude::*;

use frame_support::{
	pallet_prelude::*,
	print,
	traits::{
		fungibles::{metadata::Mutate as MetadataMutate, Inspect, Mutate, Transfer},
		Currency, EnsureOriginWithArg,
		ExistenceRequirement::KeepAlive,
		Get, Imbalance, OnUnbalanced, ReservableCurrency, WithdrawReasons,
	},
	weights::Weight,
	BoundedVec, PalletId,
};
use frame_system::pallet_prelude::OriginFor;

use dao_primitives::{DaoOrigin, DaoPolicy, DaoProvider, DaoToken, DispatchResultWithDaoOrigin};

pub use pallet::*;
pub use weights::WeightInfo;

pub type BalanceOf<T, I = ()> =
	<<T as Config<I>>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
pub type PositiveImbalanceOf<T, I = ()> = <<T as Config<I>>::Currency as Currency<
	<T as frame_system::Config>::AccountId,
>>::PositiveImbalance;
pub type NegativeImbalanceOf<T, I = ()> = <<T as Config<I>>::Currency as Currency<
	<T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

/// A trait to allow the Treasury Pallet to spend it's funds for other purposes.
/// There is an expectation that the implementer of this trait will correctly manage
/// the mutable variables passed to it:
/// * `dao_id`: DAO ID
/// * `budget_remaining`: How much available funds that can be spent by the treasury. As funds are
///   spent, you must correctly deduct from this value.
/// * `imbalance`: Any imbalances that you create should be subsumed in here to maximize efficiency
///   of updating the total issuance. (i.e. `deposit_creating`)
/// * `total_weight`: Track any weight that your `spend_fund` implementation uses by updating this
///   value.
/// * `missed_any`: If there were items that you want to spend on, but there were not enough funds,
///   mark this value as `true`. This will prevent the treasury from burning the excess funds.
#[impl_trait_for_tuples::impl_for_tuples(30)]
pub trait SpendFunds<T: Config<I>, I: 'static = ()> {
	fn spend_funds(
		dao_id: DaoId,
		budget_remaining: &mut BalanceOf<T, I>,
		imbalance: &mut PositiveImbalanceOf<T, I>,
		total_weight: &mut Weight,
		missed_any: &mut bool,
	);
}

/// Dao ID. Just a `u32`.
pub type DaoId = u32;

/// An index of a proposal. Just a `u32`.
pub type ProposalIndex = u32;

/// A spending proposal.
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[derive(Encode, Decode, Clone, PartialEq, Eq, MaxEncodedLen, RuntimeDebug, TypeInfo)]
pub struct Proposal<AccountId, Balance> {
	/// Dao ID
	dao_id: DaoId,
	/// The account proposing it.
	proposer: AccountId,
	/// The (total) amount that should be paid if the proposal is accepted.
	value: Balance,
	/// The account to whom the payment should be made if the proposal is accepted.
	beneficiary: AccountId,
	/// The amount held on deposit (reserved) for making this proposal.
	bond: Balance,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::HasCompact;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		/// The staking balance.
		type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

		type AssetId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo
			+ From<u32>
			+ Ord;

		/// Origin from which approvals must come.
		type ApproveOrigin: EnsureOriginWithArg<Self::RuntimeOrigin, DaoOrigin<Self::AccountId>>;

		/// The overarching event type.
		type RuntimeEvent: From<Event<Self, I>>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Handler for the unbalanced decrease when slashing for a rejected proposal or bounty.
		type OnSlash: OnUnbalanced<NegativeImbalanceOf<Self, I>>;

		/// Period between successive spends.
		#[pallet::constant]
		type SpendPeriod: Get<Self::BlockNumber>;

		/// Percentage of spare funds (if any) that are burnt per spend period.
		#[pallet::constant]
		type Burn: Get<Permill>;

		/// The treasury's pallet id, used for deriving its sovereign account ID.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Handler for the unbalanced decrease when treasury funds are burned.
		type BurnDestination: OnUnbalanced<NegativeImbalanceOf<Self, I>>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;

		/// Runtime hooks to external pallet using treasury to compute spend funds.
		type SpendFunds: SpendFunds<Self, I>;

		/// The maximum number of approvals that can wait in the spending queue.
		///
		/// NOTE: This parameter is also used within the Bounties Pallet extension if enabled.
		#[pallet::constant]
		type MaxApprovals: Get<u32>;

		type DaoProvider: DaoProvider<
			<Self as frame_system::Config>::Hash,
			Id = u32,
			AccountId = Self::AccountId,
			AssetId = Self::AssetId,
			Policy = DaoPolicy,
		>;

		type AssetProvider: Inspect<
				Self::AccountId,
				AssetId = <Self as pallet::Config<I>>::AssetId,
				Balance = BalanceOf<Self, I>,
			> + MetadataMutate<
				Self::AccountId,
				AssetId = <Self as pallet::Config<I>>::AssetId,
				Balance = BalanceOf<Self, I>,
			> + Mutate<
				Self::AccountId,
				AssetId = <Self as pallet::Config<I>>::AssetId,
				Balance = BalanceOf<Self, I>,
			> + Transfer<
				Self::AccountId,
				AssetId = <Self as pallet::Config<I>>::AssetId,
				Balance = BalanceOf<Self, I>,
			>;
	}

	/// Number of proposals that have been made.
	#[pallet::storage]
	#[pallet::getter(fn proposal_count)]
	pub(crate) type ProposalCount<T, I = ()> =
		StorageMap<_, Twox64Concat, DaoId, ProposalIndex, ValueQuery>;

	/// Proposals that have been made.
	#[pallet::storage]
	#[pallet::getter(fn proposals)]
	pub type Proposals<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
		_,
		Twox64Concat,
		DaoId,
		Twox64Concat,
		ProposalIndex,
		Proposal<T::AccountId, BalanceOf<T, I>>,
		OptionQuery,
	>;

	/// Proposals pending approval.
	#[pallet::storage]
	#[pallet::getter(fn pending_proposals)]
	pub type PendingProposals<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
		_,
		Twox64Concat,
		DaoId,
		Identity,
		T::Hash,
		Proposal<T::AccountId, BalanceOf<T, I>>,
		OptionQuery,
	>;

	/// Proposal indices that have been approved but not yet awarded.
	#[pallet::storage]
	#[pallet::getter(fn approvals)]
	pub type Approvals<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Twox64Concat, DaoId, BoundedVec<ProposalIndex, T::MaxApprovals>, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		/// We have ended a spend period and will now allocate funds.
		Spending { dao_id: DaoId, budget_remaining: BalanceOf<T, I> },
		/// Some funds have been allocated.
		Awarded {
			dao_id: DaoId,
			proposal_index: ProposalIndex,
			award: BalanceOf<T, I>,
			account: T::AccountId,
		},
		/// Some of our funds have been burnt.
		Burnt { dao_id: DaoId, burnt_funds: BalanceOf<T, I> },
		/// Spending has finished; this is the amount that rolls over until next spend.
		Rollover { dao_id: DaoId, rollover_balance: BalanceOf<T, I> },
		/// Some funds have been deposited.
		Deposit { value: BalanceOf<T, I> },
		/// A new spend proposal has been approved.
		SpendApproved {
			dao_id: DaoId,
			proposal_index: ProposalIndex,
			amount: BalanceOf<T, I>,
			beneficiary: T::AccountId,
		},
	}

	/// Error for the treasury pallet.
	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// Proposer's balance is too low.
		InsufficientProposersBalance,
		/// No proposal or bounty at that index.
		InvalidIndex,
		/// Too many approvals in the queue.
		TooManyApprovals,
		/// The spend origin is valid but the amount it is allowed to spend is lower than the
		/// amount to be spent.
		InsufficientPermission,
		/// Proposal has not been approved.
		ProposalNotApproved,
		NotSupported,
	}

	/// TODO: beware of huge DAO count - use chunk spend instead
	/// perhaps set another storage for pending spends per block similar to scheduler approach
	/// or use scheduler instead???
	#[pallet::hooks]
	impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {
		/// # <weight>
		/// - Complexity: `O(A)` where `A` is the number of approvals
		/// - Db reads and writes: `Approvals`, `pot account data`
		/// - Db reads and writes per approval: `Proposals`, `proposer account data`, `beneficiary
		///   account data`
		/// - The weight is overestimated if some approvals got missed.
		/// # </weight>
		fn on_initialize(n: T::BlockNumber) -> Weight {
			let mut weight = Weight::zero();
			// Check to see if we should spend some funds!
			if (n % T::SpendPeriod::get()).is_zero() {
				let dao_count = T::DaoProvider::count();

				for dao_id in 0..dao_count {
					weight += Self::spend_funds(dao_id);
				}

				weight
			} else {
				Weight::zero()
			}
		}
	}

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		/// Propose and approve a spend of treasury funds.
		///
		/// - `origin`: Must be `ApproveOrigin` with the `Success` value being at least `amount`.
		/// - `dao_id`: DAO ID.
		/// - `amount`: The amount to be transferred from the treasury to the `beneficiary`.
		/// - `beneficiary`: The destination account for the transfer.
		///
		/// NOTE: For record-keeping purposes, the proposer is deemed to be equivalent to the
		/// beneficiary.
		#[pallet::weight(T::WeightInfo::spend())]
		pub fn spend(
			origin: OriginFor<T>,
			dao_id: DaoId,
			#[pallet::compact] amount: BalanceOf<T, I>,
			beneficiary: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResult {
			Self::ensure_approved(origin, dao_id)?;

			let beneficiary = T::Lookup::lookup(beneficiary)?;

			let proposal_index = Self::proposal_count(dao_id);
			Approvals::<T, I>::try_append(dao_id, proposal_index)
				.map_err(|_| Error::<T, I>::TooManyApprovals)?;
			let proposal = Proposal {
				dao_id,
				proposer: beneficiary.clone(),
				value: amount,
				beneficiary: beneficiary.clone(),
				bond: Default::default(),
			};
			Proposals::<T, I>::insert(dao_id, proposal_index, proposal);
			ProposalCount::<T, I>::insert(dao_id, proposal_index + 1);

			Self::deposit_event(Event::SpendApproved {
				dao_id,
				proposal_index,
				amount,
				beneficiary,
			});
			Ok(())
		}

		#[pallet::weight(10_000)]
		pub fn transfer_token(
			origin: OriginFor<T>,
			dao_id: DaoId,
			#[pallet::compact] amount: BalanceOf<T, I>,
			beneficiary: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResult {
			let dao_origin = Self::ensure_approved(origin, dao_id)?;

			let dao_token = T::DaoProvider::dao_token(dao_id)?;

			match dao_token {
				DaoToken::FungibleToken(token_id) => Self::do_transfer_token(
					token_id,
					dao_origin.dao_account_id,
					amount,
					beneficiary,
				),
				DaoToken::EthTokenAddress(_) => Err(Error::<T, I>::NotSupported.into()),
			}
		}

		#[pallet::weight(10_000)]
		pub fn transfer_token_by_id(
			origin: OriginFor<T>,
			dao_id: DaoId,
			token_id: T::AssetId,
			#[pallet::compact] amount: BalanceOf<T, I>,
			beneficiary: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResult {
			let dao_origin = Self::ensure_approved(origin, dao_id)?;

			Self::do_transfer_token(token_id, dao_origin.dao_account_id, amount, beneficiary)
		}
	}
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
	// Add public immutables and private mutables.

	/// The account ID of the treasury pot.
	///
	/// This actually does computation. If you need to keep using it, then make sure you cache the
	/// value and only call this once.
	pub fn account_id() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	pub fn u128_to_balance_of(cost: u128) -> BalanceOf<T, I> {
		TryInto::<BalanceOf<T, I>>::try_into(cost).ok().unwrap()
	}

	/// Spend some money! returns number of approvals before spend.
	pub fn spend_funds(dao_id: DaoId) -> Weight {
		let mut total_weight: Weight = Zero::zero();

		let mut budget_remaining = Self::pot(dao_id);
		Self::deposit_event(Event::Spending { dao_id, budget_remaining });
		let account_id = T::DaoProvider::dao_account_id(dao_id);

		let mut missed_any = false;
		let mut imbalance = <PositiveImbalanceOf<T, I>>::zero();
		let proposals_len = Approvals::<T, I>::mutate(dao_id, |v| {
			let proposals_approvals_len = v.len() as u32;
			v.retain(|&index| {
				// Should always be true, but shouldn't panic if false or we're screwed.
				if let Some(p) = Self::proposals(dao_id, index) {
					if p.value <= budget_remaining {
						budget_remaining -= p.value;
						<Proposals<T, I>>::remove(dao_id, index);

						// return their deposit.
						let err_amount = T::Currency::unreserve(&p.proposer, p.bond);
						debug_assert!(err_amount.is_zero());

						// provide the allocation.
						imbalance.subsume(T::Currency::deposit_creating(&p.beneficiary, p.value));

						Self::deposit_event(Event::Awarded {
							dao_id,
							proposal_index: index,
							award: p.value,
							account: p.beneficiary,
						});
						false
					} else {
						missed_any = true;
						true
					}
				} else {
					false
				}
			});
			proposals_approvals_len
		});

		total_weight += T::WeightInfo::on_initialize_proposals(proposals_len);

		// Call Runtime hooks to external pallet using treasury to compute spend funds.
		T::SpendFunds::spend_funds(
			dao_id,
			&mut budget_remaining,
			&mut imbalance,
			&mut total_weight,
			&mut missed_any,
		);

		// Must never be an error, but better to be safe.
		// proof: budget_remaining is account free balance minus ED;
		// Thus we can't spend more than account free balance minus ED;
		// Thus account is kept alive; qed;
		if let Err(problem) =
			T::Currency::settle(&account_id, imbalance, WithdrawReasons::TRANSFER, KeepAlive)
		{
			print("Inconsistent state - couldn't settle imbalance for funds spent by treasury");
			// Nothing else to do here.
			drop(problem);
		}

		Self::deposit_event(Event::Rollover { dao_id, rollover_balance: budget_remaining });

		total_weight
	}

	pub fn do_transfer_token(
		token_id: T::AssetId,
		source: T::AccountId,
		amount: BalanceOf<T, I>,
		beneficiary: <T::Lookup as StaticLookup>::Source,
	) -> DispatchResult {
		let beneficiary = T::Lookup::lookup(beneficiary)?;

		T::AssetProvider::transfer(token_id, &source, &beneficiary, amount, true)?;

		Ok(())
	}

	/// Return the amount of money in the pot.
	// The existential deposit is not part of the pot so treasury account never gets deleted.
	pub fn pot(dao_id: DaoId) -> BalanceOf<T, I> {
		T::Currency::free_balance(&T::DaoProvider::dao_account_id(dao_id))
			// Must never be less than 0 but better be safe.
			.saturating_sub(T::Currency::minimum_balance())
	}

	pub fn ensure_approved(
		origin: OriginFor<T>,
		dao_id: DaoId,
	) -> DispatchResultWithDaoOrigin<T::AccountId> {
		let dao_account_id = T::DaoProvider::dao_account_id(dao_id);
		let approve_origin = T::DaoProvider::policy(dao_id)?.approve_origin;
		let dao_origin = DaoOrigin { dao_account_id, proportion: approve_origin };
		T::ApproveOrigin::ensure_origin(origin, &dao_origin)?;

		Ok(dao_origin)
	}
}

impl<T: Config<I>, I: 'static> OnUnbalanced<NegativeImbalanceOf<T, I>> for Pallet<T, I> {
	// TODO: revise for dao_id
	fn on_nonzero_unbalanced(amount: NegativeImbalanceOf<T, I>) {
		let numeric_amount = amount.peek();

		T::Currency::resolve_creating(&Self::account_id(), amount);

		Self::deposit_event(Event::Deposit { value: numeric_amount });
	}
}
