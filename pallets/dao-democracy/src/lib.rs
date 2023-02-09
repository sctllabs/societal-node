// This file is part of Substrate.

// Copyright (C) 2017-2022 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # Democracy Pallet
//!
//! - [`Config`]
//! - [`Call`]
//!
//! ## Overview
//!
//! The Democracy pallet handles the administration of general stakeholder voting.
//!
//! There are two different queues that a proposal can be added to before it
//! becomes a referendum, 1) the proposal queue consisting of all public proposals
//! and 2) the external queue consisting of a single proposal that originates
//! from one of the _external_ origins (such as a collective group).
//!
//! Every launch period - a length defined in the runtime - the Democracy pallet
//! launches a referendum from a proposal that it takes from either the proposal
//! queue or the external queue in turn. Any token holder in the system can vote
//! on referenda. The voting system
//! uses time-lock voting by allowing the token holder to set their _conviction_
//! behind a vote. The conviction will dictate the length of time the tokens
//! will be locked, as well as the multiplier that scales the vote power.
//!
//! ### Terminology
//!
//! - **Enactment Period:** The minimum period of locking and the period between a proposal being
//! approved and enacted.
//! - **Lock Period:** A period of time after proposal enactment that the tokens of _winning_ voters
//! will be locked.
//! - **Conviction:** An indication of a voter's strength of belief in their vote. An increase
//! of one in conviction indicates that a token holder is willing to lock their tokens for twice
//! as many lock periods after enactment.
//! - **Vote:** A value that can either be in approval ("Aye") or rejection ("Nay") of a particular
//!   referendum.
//! - **Proposal:** A submission to the chain that represents an action that a proposer (either an
//! account or an external origin) suggests that the system adopt.
//! - **Referendum:** A proposal that is in the process of being voted on for either acceptance or
//!   rejection as a change to the system.
//! - **Delegation:** The act of granting your voting power to the decisions of another account for
//!   up to a certain conviction.
//!
//! ### Adaptive Quorum Biasing
//!
//! A _referendum_ can be either simple majority-carries in which 50%+1 of the
//! votes decide the outcome or _adaptive quorum biased_. Adaptive quorum biasing
//! makes the threshold for passing or rejecting a referendum higher or lower
//! depending on how the referendum was originally proposed. There are two types of
//! adaptive quorum biasing: 1) _positive turnout bias_ makes a referendum
//! require a super-majority to pass that decreases as turnout increases and
//! 2) _negative turnout bias_ makes a referendum require a super-majority to
//! reject that decreases as turnout increases. Another way to think about the
//! quorum biasing is that _positive bias_ referendums will be rejected by
//! default and _negative bias_ referendums get passed by default.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! #### Public
//!
//! These calls can be made from any externally held account capable of creating
//! a signed extrinsic.
//!
//! Basic actions:
//! - `propose` - Submits a sensitive action, represented as a hash. Requires a deposit.
//! - `second` - Signals agreement with a proposal, moves it higher on the proposal queue, and
//!   requires a matching deposit to the original.
//! - `vote` - Votes in a referendum, either the vote is "Aye" to enact the proposal or "Nay" to
//!   keep the status quo.
//! - `unvote` - Cancel a previous vote, this must be done by the voter before the vote ends.
//! - `delegate` - Delegates the voting power (tokens * conviction) to another account.
//! - `undelegate` - Stops the delegation of voting power to another account.
//!
//! Administration actions that can be done to any account:
//! - `reap_vote` - Remove some account's expired votes.
//! - `unlock` - Redetermine the account's balance lock, potentially making tokens available.
//!
//! Preimage actions:
//! - `note_preimage` - Registers the preimage for an upcoming proposal, requires a deposit that is
//!   returned once the proposal is enacted.
//! - `note_preimage_operational` - same but provided by `T::OperationalPreimageOrigin`.
//! - `note_imminent_preimage` - Registers the preimage for an upcoming proposal. Does not require a
//!   deposit, but the proposal must be in the dispatch queue.
//! - `note_imminent_preimage_operational` - same but provided by `T::OperationalPreimageOrigin`.
//! - `reap_preimage` - Removes the preimage for an expired proposal. Will only work under the
//!   condition that it's the same account that noted it and after the voting period, OR it's a
//!   different account after the enactment period.
//!
//! #### Cancellation Origin
//!
//! This call can only be made by the `CancellationOrigin`.
//!
//! - `emergency_cancel` - Schedules an emergency cancellation of a referendum. Can only happen once
//!   to a specific referendum.
//!
//! #### ExternalOrigin
//!
//! This call can only be made by the `ExternalOrigin`.
//!
//! - `external_propose` - Schedules a proposal to become a referendum once it is is legal for an
//!   externally proposed referendum.
//!
//! #### External Majority Origin
//!
//! This call can only be made by the `ExternalMajorityOrigin`.
//!
//! - `external_propose_majority` - Schedules a proposal to become a majority-carries referendum
//!   once it is legal for an externally proposed referendum.
//!
//! #### External Default Origin
//!
//! This call can only be made by the `ExternalDefaultOrigin`.
//!
//! - `external_propose_default` - Schedules a proposal to become a negative-turnout-bias referendum
//!   once it is legal for an externally proposed referendum.
//!
//! #### Fast Track Origin
//!
//! This call can only be made by the `FastTrackOrigin`.
//!
//! - `fast_track` - Schedules the current externally proposed proposal that is "majority-carries"
//!   to become a referendum immediately.
//!
//! #### Veto Origin
//!
//! This call can only be made by the `VetoOrigin`.
//!
//! - `veto_external` - Vetoes and blacklists the external proposal hash.
//!
//! #### Root
//!
//! - `cancel_referendum` - Removes a referendum.
//! - `cancel_queued` - Cancels a proposal that is queued for enactment.
//! - `clear_public_proposal` - Removes all public proposals.

#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use dao_primitives::{
	DaoGovernance, DaoOrigin, DaoPolicy, DaoProvider, DaoToken, GovernanceV1Policy, RawOrigin,
};
use frame_support::{
	ensure,
	traits::{
		defensive_prelude::*,
		fungibles::{BalancedHold, Inspect, InspectHold, MutateHold},
		schedule::{v3::Named as ScheduleNamed, DispatchTime},
		Bounded, Currency, Get, LockIdentifier, OnUnbalanced, QueryPreimage, StorePreimage,
	},
	weights::Weight,
};
use frame_system::pallet_prelude::OriginFor;
use sp_runtime::{
	traits::{Bounded as ArithBounded, One, Saturating, StaticLookup, Zero},
	ArithmeticError, DispatchError, DispatchResult,
};
use sp_std::prelude::*;

mod conviction;
mod types;
mod vote;
pub mod vote_threshold;
pub mod weights;
pub use conviction::Conviction;
pub use pallet::*;
use pallet_dao_assets::LockableAsset;
pub use types::{Delegations, ReferendumInfo, ReferendumStatus, Tally, UnvoteScope};
pub use vote::{AccountVote, Vote, Voting};
pub use vote_threshold::{Approved, VoteThreshold};
pub use weights::WeightInfo;

#[cfg(test)]
#[cfg(feature = "dao_democracy_tests")]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

const DEMOCRACY_ID: LockIdentifier = *b"democrac";

/// Dao ID. Just a `u32`.
pub type DaoId = u32;

/// A proposal index.
pub type PropIndex = u32;

/// A referendum index.
pub type ReferendumIndex = u32;

type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
	<T as frame_system::Config>::AccountId,
>>::NegativeImbalance;
pub type CallOf<T> = <T as frame_system::Config>::RuntimeCall;
pub type BoundedCallOf<T> = Bounded<CallOf<T>>;
type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

#[frame_support::pallet]
pub mod pallet {
	use super::{DispatchResult, *};
	use frame_support::{
		pallet_prelude::*,
		traits::{EnsureOriginWithArg, TryDrop},
	};
	use frame_system::pallet_prelude::*;
	use sp_core::H256;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + Sized {
		type WeightInfo: WeightInfo;
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The Scheduler.
		type Scheduler: ScheduleNamed<Self::BlockNumber, CallOf<Self>, Self::PalletsOrigin>;

		/// The Preimage provider.
		type Preimages: QueryPreimage + StorePreimage;

		/// Currency type for this pallet.
		type Currency: Currency<Self::AccountId>;

		type Assets: LockableAsset<
				Self::AccountId,
				AssetId = u32,
				Moment = Self::BlockNumber,
				Balance = BalanceOf<Self>,
			> + Inspect<Self::AccountId>
			+ InspectHold<Self::AccountId>
			+ MutateHold<Self::AccountId>
			+ BalancedHold<Self::AccountId>;

		/// The maximum number of votes for an account.
		///
		/// Also used to compute weight, an overly big value can
		/// lead to extrinsic with very big weight: see `delegate` for instance.
		#[pallet::constant]
		type MaxVotes: Get<u32>;

		/// The maximum number of public proposals that can exist at any time.
		#[pallet::constant]
		type MaxProposals: Get<u32>;

		/// The maximum number of deposits a public proposal may have at any time.
		#[pallet::constant]
		type MaxDeposits: Get<u32>;

		/// The maximum number of items which can be blacklisted.
		#[pallet::constant]
		type MaxBlacklisted: Get<u32>;

		#[pallet::constant]
		type ProposalMetadataLimit: Get<u32>;

		/// Origin from which the next tabled referendum may be forced. This is a normal
		/// "super-majority-required" referendum.
		type ExternalOrigin: EnsureOriginWithArg<Self::RuntimeOrigin, DaoOrigin<Self::AccountId>>;

		/// Origin from which the next tabled referendum may be forced; this allows for the tabling
		/// of a majority-carries referendum.
		type ExternalMajorityOrigin: EnsureOriginWithArg<
			Self::RuntimeOrigin,
			DaoOrigin<Self::AccountId>,
		>;

		/// Origin from which the next tabled referendum may be forced; this allows for the tabling
		/// of a negative-turnout-bias (default-carries) referendum.
		type ExternalDefaultOrigin: EnsureOriginWithArg<
			Self::RuntimeOrigin,
			DaoOrigin<Self::AccountId>,
		>;

		/// Origin from which the next majority-carries (or more permissive) referendum may be
		/// tabled to vote according to the `FastTrackVotingPeriod` asynchronously in a similar
		/// manner to the emergency origin. It retains its threshold method.
		type FastTrackOrigin: EnsureOriginWithArg<Self::RuntimeOrigin, DaoOrigin<Self::AccountId>>;

		/// Origin from which the next majority-carries (or more permissive) referendum may be
		/// tabled to vote immediately and asynchronously in a similar manner to the emergency
		/// origin. It retains its threshold method.
		type InstantOrigin: EnsureOriginWithArg<Self::RuntimeOrigin, DaoOrigin<Self::AccountId>>;

		/// Origin from which any referendum may be cancelled in an emergency.
		type CancellationOrigin: EnsureOriginWithArg<
			Self::RuntimeOrigin,
			DaoOrigin<Self::AccountId>,
		>;

		/// Origin from which proposals may be blacklisted.
		type BlacklistOrigin: EnsureOriginWithArg<Self::RuntimeOrigin, DaoOrigin<Self::AccountId>>;

		/// Origin from which a proposal may be cancelled and its backers slashed.
		type CancelProposalOrigin: EnsureOriginWithArg<
			Self::RuntimeOrigin,
			DaoOrigin<Self::AccountId>,
		>;

		/// Origin for anyone able to veto proposals.
		type VetoOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = Self::AccountId>;

		/// Overarching type of all pallets origins.
		type PalletsOrigin: From<RawOrigin<Self::AccountId>>;

		/// Handler for the unbalanced reduction when slashing a preimage deposit.
		type Slash: OnUnbalanced<NegativeImbalanceOf<Self>>;

		type DaoProvider: DaoProvider<
			<Self as frame_system::Config>::Hash,
			Id = u32,
			AccountId = Self::AccountId,
			AssetId = u32,
			Policy = DaoPolicy,
		>;
	}

	/// The number of (public) proposals that have been made so far.
	#[pallet::storage]
	#[pallet::getter(fn public_prop_count)]
	pub type PublicPropCount<T> = StorageMap<_, Twox64Concat, DaoId, PropIndex, ValueQuery>;

	/// The public proposals. Unsorted. The second item is the proposal.
	#[pallet::storage]
	#[pallet::getter(fn public_props)]
	pub type PublicProps<T: Config> = StorageMap<
		_,
		Twox64Concat,
		DaoId,
		BoundedVec<(PropIndex, BoundedCallOf<T>, T::AccountId), T::MaxProposals>,
		ValueQuery,
	>;

	/// Those who have locked a deposit.
	///
	/// TWOX-NOTE: Safe, as increasing integer keys are safe.
	#[pallet::storage]
	#[pallet::getter(fn deposit_of)]
	pub type DepositOf<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		DaoId,
		Twox64Concat,
		PropIndex,
		(BoundedVec<T::AccountId, T::MaxDeposits>, BalanceOf<T>),
	>;

	/// The next free referendum index, aka the number of referenda started so far.
	#[pallet::storage]
	#[pallet::getter(fn referendum_count)]
	pub type ReferendumCount<T> = StorageMap<_, Twox64Concat, DaoId, ReferendumIndex, ValueQuery>;

	/// The lowest referendum index representing an unbaked referendum. Equal to
	/// `ReferendumCount` if there isn't a unbaked referendum.
	#[pallet::storage]
	#[pallet::getter(fn lowest_unbaked)]
	pub type LowestUnbaked<T> = StorageMap<_, Twox64Concat, DaoId, ReferendumIndex, ValueQuery>;

	/// Information concerning any given referendum.
	///
	/// TWOX-NOTE: SAFE as indexes are not under an attacker’s control.
	#[pallet::storage]
	#[pallet::getter(fn referendum_info)]
	pub type ReferendumInfoOf<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		DaoId,
		Twox64Concat,
		ReferendumIndex,
		ReferendumInfo<T::BlockNumber, BoundedCallOf<T>, BalanceOf<T>>,
	>;

	/// All votes for a particular voter. We store the balance for the number of votes that we
	/// have recorded. The second item is the total amount of delegations, that will be added.
	///
	/// TWOX-NOTE: SAFE as `AccountId`s are crypto hashes anyway.
	#[pallet::storage]
	pub type VotingOf<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		DaoId,
		Twox64Concat,
		T::AccountId,
		Voting<BalanceOf<T>, T::AccountId, T::BlockNumber, T::MaxVotes>,
		ValueQuery,
	>;

	/// True if the last referendum tabled was submitted externally. False if it was a public
	/// proposal.
	#[pallet::storage]
	pub type LastTabledWasExternal<T> = StorageMap<_, Twox64Concat, DaoId, bool, ValueQuery>;

	/// The referendum to be tabled whenever it would be valid to table an external proposal.
	/// This happens when a referendum needs to be tabled and one of two conditions are met:
	/// - `LastTabledWasExternal` is `false`; or
	/// - `PublicProps` is empty.
	#[pallet::storage]
	pub type NextExternal<T: Config> =
		StorageMap<_, Twox64Concat, DaoId, (BoundedCallOf<T>, VoteThreshold)>;

	/// A record of who vetoed what. Maps proposal hash to a possible existent block number
	/// (until when it may not be resubmitted) and who vetoed it.
	#[pallet::storage]
	pub type Blacklist<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		DaoId,
		Identity,
		H256,
		(T::BlockNumber, BoundedVec<T::AccountId, T::MaxBlacklisted>),
	>;

	/// Record of all proposals that have been subject to emergency cancellation.
	#[pallet::storage]
	pub type Cancellations<T: Config> =
		StorageDoubleMap<_, Twox64Concat, DaoId, Identity, H256, bool, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A motion has been proposed by a public account.
		Proposed {
			dao_id: DaoId,
			proposal_index: PropIndex,
			deposit: BalanceOf<T>,
			meta: Option<Vec<u8>>,
		},
		/// A public proposal has been tabled for referendum vote.
		Tabled { dao_id: DaoId, proposal_index: PropIndex, deposit: BalanceOf<T> },
		/// An external proposal has been tabled.
		ExternalTabled,
		/// A referendum has begun.
		Started { dao_id: DaoId, ref_index: ReferendumIndex, threshold: VoteThreshold },
		/// A proposal has been approved by referendum.
		Passed { dao_id: DaoId, ref_index: ReferendumIndex },
		/// A proposal has been rejected by referendum.
		NotPassed { dao_id: DaoId, ref_index: ReferendumIndex },
		/// A referendum has been cancelled.
		Cancelled { dao_id: DaoId, ref_index: ReferendumIndex },
		/// An account has delegated their vote to another account.
		Delegated { dao_id: DaoId, who: T::AccountId, target: T::AccountId },
		/// An account has cancelled a previous delegation operation.
		Undelegated { dao_id: DaoId, account: T::AccountId },
		/// An external proposal has been vetoed.
		Vetoed { dao_id: DaoId, who: T::AccountId, proposal_hash: H256, until: T::BlockNumber },
		/// A proposal_hash has been blacklisted permanently.
		Blacklisted { dao_id: DaoId, proposal_hash: H256 },
		/// An account has voted in a referendum
		Voted {
			dao_id: DaoId,
			voter: T::AccountId,
			ref_index: ReferendumIndex,
			vote: AccountVote<BalanceOf<T>>,
		},
		/// An account has secconded a proposal
		Seconded { dao_id: DaoId, seconder: T::AccountId, prop_index: PropIndex },
		/// A proposal got canceled.
		ProposalCanceled { dao_id: DaoId, prop_index: PropIndex },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Value too low
		ValueLow,
		/// Proposal does not exist
		ProposalMissing,
		/// Cannot cancel the same proposal twice
		AlreadyCanceled,
		/// Proposal already made
		DuplicateProposal,
		/// Proposal still blacklisted
		ProposalBlacklisted,
		/// Next external proposal not simple majority
		NotSimpleMajority,
		/// Invalid hash
		InvalidHash,
		/// No external proposal
		NoProposal,
		/// Identity may not veto a proposal twice
		AlreadyVetoed,
		/// Vote given for invalid referendum
		ReferendumInvalid,
		/// No proposals waiting
		NoneWaiting,
		/// The given account did not vote on the referendum.
		NotVoter,
		/// The actor has no permission to conduct the action.
		NoPermission,
		/// The account is already delegating.
		AlreadyDelegating,
		/// Too high a balance was provided that the account cannot afford.
		InsufficientFunds,
		/// The account is not currently delegating.
		NotDelegating,
		/// The account currently has votes attached to it and the operation cannot succeed until
		/// these are removed, either through `unvote` or `reap_vote`.
		VotesExist,
		/// The instant referendum origin is currently disallowed.
		InstantNotAllowed,
		/// Delegation to oneself makes no sense.
		Nonsense,
		/// Invalid upper bound.
		WrongUpperBound,
		/// Maximum number of votes reached.
		MaxVotesReached,
		/// Maximum number of items reached.
		TooMany,
		/// Voting period too low
		VotingPeriodLow,
		/// This type of Governance is not supported
		NotSupported,
		/// Metadata size exceeds the limits
		MetadataTooLong,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		/// Weight: see `begin_block`
		fn on_initialize(n: T::BlockNumber) -> Weight {
			Self::begin_block(n)
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Propose a sensitive action to be taken.
		///
		/// The dispatch origin of this call must be _Signed_ and the sender must
		/// have funds to cover the deposit.
		///
		/// - `proposal_hash`: The hash of the proposal preimage.
		/// - `value`: The amount of deposit (must be at least `MinimumDeposit`).
		///
		/// Emits `Proposed`.
		#[pallet::weight(T::WeightInfo::propose())]
		pub fn propose(
			origin: OriginFor<T>,
			dao_id: DaoId,
			proposal: BoundedCallOf<T>,
			#[pallet::compact] value: BalanceOf<T>,
		) -> DispatchResult {
			Self::propose_with_meta(origin, dao_id, proposal, value, None)
		}

		/// Adds a new proposal with temporary meta field for arbitrary data indexed by node indexer
		#[pallet::weight(T::WeightInfo::propose())]
		pub fn propose_with_meta(
			origin: OriginFor<T>,
			dao_id: DaoId,
			proposal: BoundedCallOf<T>,
			#[pallet::compact] value: BalanceOf<T>,
			meta: Option<Vec<u8>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let GovernanceV1Policy { minimum_deposit, .. } =
				Self::ensure_democracy_supported(dao_id)?;

			if let Some(metadata) = meta.clone() {
				ensure!(
					BoundedVec::<u8, T::ProposalMetadataLimit>::try_from(metadata).is_ok(),
					Error::<T>::MetadataTooLong
				)
			}

			ensure!(value >= Self::u128_to_balance_of(minimum_deposit), Error::<T>::ValueLow);

			let index = Self::public_prop_count(dao_id);
			let real_prop_count = PublicProps::<T>::decode_len(dao_id).unwrap_or(0) as u32;
			let max_proposals = T::MaxProposals::get();
			ensure!(real_prop_count < max_proposals, Error::<T>::TooMany);
			let proposal_hash = proposal.hash();

			if let Some((until, _)) = <Blacklist<T>>::get(dao_id, proposal_hash) {
				ensure!(
					<frame_system::Pallet<T>>::block_number() >= until,
					Error::<T>::ProposalBlacklisted,
				);
			}

			let dao_token = T::DaoProvider::dao_token(dao_id)?;
			match dao_token {
				DaoToken::FungibleToken(token_id) => T::Assets::hold(token_id, &who, value)?,
				DaoToken::EthTokenAddress(_) => {},
			}

			let depositors = BoundedVec::<_, T::MaxDeposits>::truncate_from(vec![who.clone()]);
			DepositOf::<T>::insert(dao_id, index, (depositors, value));

			PublicPropCount::<T>::insert(dao_id, index + 1);

			PublicProps::<T>::try_append(dao_id, (index, proposal, who))
				.map_err(|_| Error::<T>::TooMany)?;

			Self::deposit_event(Event::<T>::Proposed {
				dao_id,
				proposal_index: index,
				deposit: value,
				meta,
			});
			Ok(())
		}

		/// Signals agreement with a particular proposal.
		///
		/// The dispatch origin of this call must be _Signed_ and the sender
		/// must have funds to cover the deposit, equal to the original deposit.
		///
		/// - `proposal`: The index of the proposal to second.
		#[pallet::weight(T::WeightInfo::second())]
		pub fn second(
			origin: OriginFor<T>,
			dao_id: DaoId,
			#[pallet::compact] proposal: PropIndex,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let seconds =
				Self::len_of_deposit_of(dao_id, proposal).ok_or(Error::<T>::ProposalMissing)?;
			ensure!(seconds < T::MaxDeposits::get(), Error::<T>::TooMany);
			let mut deposit =
				Self::deposit_of(dao_id, proposal).ok_or(Error::<T>::ProposalMissing)?;

			let dao_token = T::DaoProvider::dao_token(dao_id)?;
			match dao_token {
				DaoToken::FungibleToken(token_id) => T::Assets::hold(token_id, &who, deposit.1)?,
				DaoToken::EthTokenAddress(_) => {},
			}

			let ok = deposit.0.try_push(who.clone()).is_ok();
			debug_assert!(ok, "`seconds` is below static limit; `try_insert` should succeed; qed");
			<DepositOf<T>>::insert(dao_id, proposal, deposit);
			Self::deposit_event(Event::<T>::Seconded {
				dao_id,
				seconder: who,
				prop_index: proposal,
			});
			Ok(())
		}

		/// Vote in a referendum. If `vote.is_aye()`, the vote is to enact the proposal;
		/// otherwise it is a vote to keep the status quo.
		///
		/// The dispatch origin of this call must be _Signed_.
		///
		/// - `ref_index`: The index of the referendum to vote for.
		/// - `vote`: The vote configuration.
		#[pallet::weight(T::WeightInfo::vote_new().max(T::WeightInfo::vote_existing()))]
		pub fn vote(
			origin: OriginFor<T>,
			dao_id: DaoId,
			#[pallet::compact] ref_index: ReferendumIndex,
			vote: AccountVote<BalanceOf<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::try_vote(dao_id, &who, ref_index, vote)
		}

		/// Schedule an emergency cancellation of a referendum. Cannot happen twice to the same
		/// referendum.
		///
		/// The dispatch origin of this call must be `CancellationOrigin`.
		///
		/// -`ref_index`: The index of the referendum to cancel.
		///
		/// Weight: `O(1)`.
		#[pallet::weight((T::WeightInfo::emergency_cancel(), DispatchClass::Operational))]
		pub fn emergency_cancel(
			origin: OriginFor<T>,
			dao_id: DaoId,
			ref_index: ReferendumIndex,
		) -> DispatchResult {
			let GovernanceV1Policy { cancellation_origin, .. } =
				Self::ensure_democracy_supported(dao_id)?;

			let dao_account_id = T::DaoProvider::dao_account_id(dao_id);
			T::CancellationOrigin::ensure_origin(
				origin,
				&DaoOrigin { dao_account_id, proportion: cancellation_origin },
			)?;

			let status = Self::referendum_status(dao_id, ref_index)?;
			let h = status.proposal.hash();
			ensure!(!<Cancellations<T>>::contains_key(dao_id, h), Error::<T>::AlreadyCanceled);

			<Cancellations<T>>::insert(dao_id, h, true);
			Self::internal_cancel_referendum(dao_id, ref_index);
			Ok(())
		}

		/// Schedule a referendum to be tabled once it is legal to schedule an external
		/// referendum.
		///
		/// The dispatch origin of this call must be `ExternalOrigin`.
		///
		/// - `proposal_hash`: The preimage hash of the proposal.
		#[pallet::weight(T::WeightInfo::external_propose())]
		pub fn external_propose(
			origin: OriginFor<T>,
			dao_id: DaoId,
			proposal: BoundedCallOf<T>,
		) -> DispatchResult {
			let GovernanceV1Policy { external_origin, .. } =
				Self::ensure_democracy_supported(dao_id)?;

			let dao_account_id = T::DaoProvider::dao_account_id(dao_id);
			T::ExternalOrigin::ensure_origin(
				origin,
				&DaoOrigin { dao_account_id, proportion: external_origin },
			)?;

			ensure!(!<NextExternal<T>>::contains_key(dao_id), Error::<T>::DuplicateProposal);
			if let Some((until, _)) = <Blacklist<T>>::get(dao_id, proposal.hash()) {
				ensure!(
					<frame_system::Pallet<T>>::block_number() >= until,
					Error::<T>::ProposalBlacklisted,
				);
			}
			<NextExternal<T>>::insert(dao_id, (proposal, VoteThreshold::SuperMajorityApprove));
			Ok(())
		}

		/// Schedule a majority-carries referendum to be tabled next once it is legal to schedule
		/// an external referendum.
		///
		/// The dispatch of this call must be `ExternalMajorityOrigin`.
		///
		/// - `proposal_hash`: The preimage hash of the proposal.
		///
		/// Unlike `external_propose`, blacklisting has no effect on this and it may replace a
		/// pre-scheduled `external_propose` call.
		///
		/// Weight: `O(1)`
		#[pallet::weight(T::WeightInfo::external_propose_majority())]
		pub fn external_propose_majority(
			origin: OriginFor<T>,
			dao_id: DaoId,
			proposal: BoundedCallOf<T>,
		) -> DispatchResult {
			let GovernanceV1Policy { external_majority_origin, .. } =
				Self::ensure_democracy_supported(dao_id)?;

			let dao_account_id = T::DaoProvider::dao_account_id(dao_id);
			T::ExternalMajorityOrigin::ensure_origin(
				origin,
				&DaoOrigin { dao_account_id, proportion: external_majority_origin },
			)?;

			<NextExternal<T>>::insert(dao_id, (proposal, VoteThreshold::SimpleMajority));
			Ok(())
		}

		/// Schedule a negative-turnout-bias referendum to be tabled next once it is legal to
		/// schedule an external referendum.
		///
		/// The dispatch of this call must be `ExternalDefaultOrigin`.
		///
		/// - `proposal_hash`: The preimage hash of the proposal.
		///
		/// Unlike `external_propose`, blacklisting has no effect on this and it may replace a
		/// pre-scheduled `external_propose` call.
		///
		/// Weight: `O(1)`
		#[pallet::weight(T::WeightInfo::external_propose_default())]
		pub fn external_propose_default(
			origin: OriginFor<T>,
			dao_id: DaoId,
			proposal: BoundedCallOf<T>,
		) -> DispatchResult {
			let GovernanceV1Policy { external_default_origin, .. } =
				Self::ensure_democracy_supported(dao_id)?;

			let dao_account_id = T::DaoProvider::dao_account_id(dao_id);
			T::ExternalDefaultOrigin::ensure_origin(
				origin,
				&DaoOrigin { dao_account_id, proportion: external_default_origin },
			)?;

			<NextExternal<T>>::insert(dao_id, (proposal, VoteThreshold::SuperMajorityAgainst));
			Ok(())
		}

		/// Schedule the currently externally-proposed majority-carries referendum to be tabled
		/// immediately. If there is no externally-proposed referendum currently, or if there is one
		/// but it is not a majority-carries referendum then it fails.
		///
		/// The dispatch of this call must be `FastTrackOrigin`.
		///
		/// - `proposal_hash`: The hash of the current external proposal.
		/// - `voting_period`: The period that is allowed for voting on this proposal. Increased to
		/// 	Must be always greater than zero.
		/// 	For `FastTrackOrigin` must be equal or greater than `FastTrackVotingPeriod`.
		/// - `delay`: The number of block after voting has ended in approval and this should be
		///   enacted. This doesn't have a minimum amount.
		///
		/// Emits `Started`.
		///
		/// Weight: `O(1)`
		#[pallet::weight(T::WeightInfo::fast_track())]
		pub fn fast_track(
			origin: OriginFor<T>,
			dao_id: DaoId,
			proposal_hash: H256,
			voting_period: T::BlockNumber,
			delay: T::BlockNumber,
		) -> DispatchResult {
			let GovernanceV1Policy {
				fast_track_origin,
				fast_track_voting_period,
				instant_origin,
				instant_allowed,
				..
			} = Self::ensure_democracy_supported(dao_id)?;

			let dao_account_id = T::DaoProvider::dao_account_id(dao_id);

			let mut dao_origin = DaoOrigin { dao_account_id, proportion: fast_track_origin };

			// Rather complicated bit of code to ensure that either:
			// - `voting_period` is at least `FastTrackVotingPeriod` and `origin` is
			//   `FastTrackOrigin`; or
			// - `InstantAllowed` is `true` and `origin` is `InstantOrigin`.
			let maybe_ensure_instant =
				if voting_period < Self::u32_to_block_number(fast_track_voting_period) {
					Some(origin)
				} else if let Err(origin) = T::FastTrackOrigin::try_origin(origin, &dao_origin) {
					Some(origin)
				} else {
					None
				};
			if let Some(ensure_instant) = maybe_ensure_instant {
				dao_origin.proportion = instant_origin;
				T::InstantOrigin::ensure_origin(ensure_instant, &dao_origin)?;
				ensure!(instant_allowed, Error::<T>::InstantNotAllowed);
			}

			ensure!(voting_period > T::BlockNumber::zero(), Error::<T>::VotingPeriodLow);
			let (ext_proposal, threshold) =
				<NextExternal<T>>::get(dao_id).ok_or(Error::<T>::ProposalMissing)?;
			ensure!(
				threshold != VoteThreshold::SuperMajorityApprove,
				Error::<T>::NotSimpleMajority,
			);
			ensure!(proposal_hash == ext_proposal.hash(), Error::<T>::InvalidHash);

			<NextExternal<T>>::remove(dao_id);
			let now = <frame_system::Pallet<T>>::block_number();
			Self::inject_referendum(
				dao_id,
				now.saturating_add(voting_period),
				ext_proposal,
				threshold,
				delay,
			);
			Ok(())
		}

		/// Veto and blacklist the external proposal hash.
		///
		/// The dispatch origin of this call must be `VetoOrigin`.
		///
		/// - `proposal_hash`: The preimage hash of the proposal to veto and blacklist.
		///
		/// Emits `Vetoed`.
		///
		/// Weight: `O(V + log(V))` where V is number of `existing vetoers`
		#[pallet::weight(T::WeightInfo::veto_external())]
		pub fn veto_external(
			origin: OriginFor<T>,
			dao_id: DaoId,
			proposal_hash: H256,
		) -> DispatchResult {
			let who = T::VetoOrigin::ensure_origin(origin)?;

			let GovernanceV1Policy { cooloff_period, .. } =
				Self::ensure_democracy_supported(dao_id)?;

			if let Some((ext_proposal, _)) = NextExternal::<T>::get(dao_id) {
				ensure!(proposal_hash == ext_proposal.hash(), Error::<T>::ProposalMissing);
			} else {
				return Err(Error::<T>::NoProposal.into())
			}

			let mut existing_vetoers = <Blacklist<T>>::get(dao_id, proposal_hash)
				.map(|pair| pair.1)
				.unwrap_or_default();
			let insert_position =
				existing_vetoers.binary_search(&who).err().ok_or(Error::<T>::AlreadyVetoed)?;
			existing_vetoers
				.try_insert(insert_position, who.clone())
				.map_err(|_| Error::<T>::TooMany)?;

			let until = <frame_system::Pallet<T>>::block_number()
				.saturating_add(Self::u32_to_block_number(cooloff_period));
			<Blacklist<T>>::insert(dao_id, proposal_hash, (until, existing_vetoers));

			Self::deposit_event(Event::<T>::Vetoed { dao_id, who, proposal_hash, until });
			<NextExternal<T>>::remove(dao_id);
			Ok(())
		}

		// TODO: no roots allowed
		/// Remove a referendum.
		///
		/// The dispatch origin of this call must be _Root_.
		///
		/// - `ref_index`: The index of the referendum to cancel.
		///
		/// # Weight: `O(1)`.
		#[pallet::weight(T::WeightInfo::cancel_referendum())]
		pub fn cancel_referendum(
			origin: OriginFor<T>,
			dao_id: DaoId,
			#[pallet::compact] ref_index: ReferendumIndex,
		) -> DispatchResult {
			ensure_root(origin)?;
			Self::internal_cancel_referendum(dao_id, ref_index);
			Ok(())
		}

		/// Delegate the voting power (with some given conviction) of the sending account.
		///
		/// The balance delegated is locked for as long as it's delegated, and thereafter for the
		/// time appropriate for the conviction's lock period.
		///
		/// The dispatch origin of this call must be _Signed_, and the signing account must either:
		///   - be delegating already; or
		///   - have no voting activity (if there is, then it will need to be removed/consolidated
		///     through `reap_vote` or `unvote`).
		///
		/// - `to`: The account whose voting the `target` account's voting power will follow.
		/// - `conviction`: The conviction that will be attached to the delegated votes. When the
		///   account is undelegated, the funds will be locked for the corresponding period.
		/// - `balance`: The amount of the account's balance to be used in delegating. This must not
		///   be more than the account's current balance.
		///
		/// Emits `Delegated`.
		///
		/// Weight: `O(R)` where R is the number of referendums the voter delegating to has
		///   voted on. Weight is charged as if maximum votes.
		// NOTE: weight must cover an incorrect voting of origin with max votes, this is ensure
		// because a valid delegation cover decoding a direct voting with max votes.
		#[pallet::weight(T::WeightInfo::delegate(T::MaxVotes::get()))]
		pub fn delegate(
			origin: OriginFor<T>,
			dao_id: DaoId,
			to: AccountIdLookupOf<T>,
			conviction: Conviction,
			balance: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let to = T::Lookup::lookup(to)?;
			let votes = Self::try_delegate(dao_id, who, to, conviction, balance)?;

			Ok(Some(T::WeightInfo::delegate(votes)).into())
		}

		/// Undelegate the voting power of the sending account.
		///
		/// Tokens may be unlocked following once an amount of time consistent with the lock period
		/// of the conviction with which the delegation was issued.
		///
		/// The dispatch origin of this call must be _Signed_ and the signing account must be
		/// currently delegating.
		///
		/// Emits `Undelegated`.
		///
		/// Weight: `O(R)` where R is the number of referendums the voter delegating to has
		///   voted on. Weight is charged as if maximum votes.
		// NOTE: weight must cover an incorrect voting of origin with max votes, this is ensure
		// because a valid delegation cover decoding a direct voting with max votes.
		#[pallet::weight(T::WeightInfo::undelegate(T::MaxVotes::get()))]
		pub fn undelegate(origin: OriginFor<T>, dao_id: DaoId) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let votes = Self::try_undelegate(dao_id, who)?;
			Ok(Some(T::WeightInfo::undelegate(votes)).into())
		}

		// TODO: no roots allowed
		/// Clears all public proposals.
		///
		/// The dispatch origin of this call must be _Root_.
		///
		/// Weight: `O(1)`.
		#[pallet::weight(T::WeightInfo::clear_public_proposals())]
		pub fn clear_public_proposals(origin: OriginFor<T>, dao_id: DaoId) -> DispatchResult {
			ensure_root(origin)?;
			<PublicProps<T>>::remove(dao_id);
			Ok(())
		}

		/// Unlock tokens that have an expired lock.
		///
		/// The dispatch origin of this call must be _Signed_.
		///
		/// - `target`: The account to remove the lock on.
		///
		/// Weight: `O(R)` with R number of vote of target.
		#[pallet::weight(T::WeightInfo::unlock_set(T::MaxVotes::get()).max(T::WeightInfo::unlock_remove(T::MaxVotes::get())))]
		pub fn unlock(
			origin: OriginFor<T>,
			dao_id: DaoId,
			target: AccountIdLookupOf<T>,
		) -> DispatchResult {
			ensure_signed(origin)?;
			let target = T::Lookup::lookup(target)?;
			Self::update_lock(dao_id, &target, T::DaoProvider::dao_token(dao_id)?);
			Ok(())
		}

		/// Remove a vote for a referendum.
		///
		/// If:
		/// - the referendum was cancelled, or
		/// - the referendum is ongoing, or
		/// - the referendum has ended such that
		///   - the vote of the account was in opposition to the result; or
		///   - there was no conviction to the account's vote; or
		///   - the account made a split vote
		/// ...then the vote is removed cleanly and a following call to `unlock` may result in more
		/// funds being available.
		///
		/// If, however, the referendum has ended and:
		/// - it finished corresponding to the vote of the account, and
		/// - the account made a standard vote with conviction, and
		/// - the lock period of the conviction is not over
		/// ...then the lock will be aggregated into the overall account's lock, which may involve
		/// *overlocking* (where the two locks are combined into a single lock that is the maximum
		/// of both the amount locked and the time is it locked for).
		///
		/// The dispatch origin of this call must be _Signed_, and the signer must have a vote
		/// registered for referendum `index`.
		///
		/// - `index`: The index of referendum of the vote to be removed.
		///
		/// Weight: `O(R + log R)` where R is the number of referenda that `target` has voted on.
		///   Weight is calculated for the maximum number of vote.
		#[pallet::weight(T::WeightInfo::remove_vote(T::MaxVotes::get()))]
		pub fn remove_vote(
			origin: OriginFor<T>,
			dao_id: DaoId,
			index: ReferendumIndex,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::try_remove_vote(dao_id, &who, index, UnvoteScope::Any)
		}

		/// Remove a vote for a referendum.
		///
		/// If the `target` is equal to the signer, then this function is exactly equivalent to
		/// `remove_vote`. If not equal to the signer, then the vote must have expired,
		/// either because the referendum was cancelled, because the voter lost the referendum or
		/// because the conviction period is over.
		///
		/// The dispatch origin of this call must be _Signed_.
		///
		/// - `target`: The account of the vote to be removed; this account must have voted for
		///   referendum `index`.
		/// - `index`: The index of referendum of the vote to be removed.
		///
		/// Weight: `O(R + log R)` where R is the number of referenda that `target` has voted on.
		///   Weight is calculated for the maximum number of vote.
		#[pallet::weight(T::WeightInfo::remove_other_vote(T::MaxVotes::get()))]
		pub fn remove_other_vote(
			origin: OriginFor<T>,
			dao_id: DaoId,
			target: AccountIdLookupOf<T>,
			index: ReferendumIndex,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let target = T::Lookup::lookup(target)?;
			let scope = if target == who { UnvoteScope::Any } else { UnvoteScope::OnlyExpired };
			Self::try_remove_vote(dao_id, &target, index, scope)?;
			Ok(())
		}

		/// Permanently place a proposal into the blacklist. This prevents it from ever being
		/// proposed again.
		///
		/// If called on a queued public or external proposal, then this will result in it being
		/// removed. If the `ref_index` supplied is an active referendum with the proposal hash,
		/// then it will be cancelled.
		///
		/// The dispatch origin of this call must be `BlacklistOrigin`.
		///
		/// - `proposal_hash`: The proposal hash to blacklist permanently.
		/// - `ref_index`: An ongoing referendum whose hash is `proposal_hash`, which will be
		/// cancelled.
		///
		/// Weight: `O(p)` (though as this is an high-privilege dispatch, we assume it has a
		///   reasonable value).
		#[pallet::weight((T::WeightInfo::blacklist(), DispatchClass::Operational))]
		pub fn blacklist(
			origin: OriginFor<T>,
			dao_id: DaoId,
			proposal_hash: H256,
			maybe_ref_index: Option<ReferendumIndex>,
		) -> DispatchResult {
			let dao_account_id = T::DaoProvider::dao_account_id(dao_id);

			let GovernanceV1Policy { blacklist_origin, .. } =
				Self::ensure_democracy_supported(dao_id)?;

			T::BlacklistOrigin::ensure_origin(
				origin,
				&DaoOrigin { dao_account_id, proportion: blacklist_origin },
			)?;

			let dao_token = T::DaoProvider::dao_token(dao_id)?;

			// Insert the proposal into the blacklist.
			let permanent = (T::BlockNumber::max_value(), BoundedVec::<T::AccountId, _>::default());
			Blacklist::<T>::insert(dao_id, proposal_hash, permanent);

			// Remove the queued proposal, if it's there.
			PublicProps::<T>::mutate(dao_id, |props| {
				if let Some(index) = props.iter().position(|p| p.1.hash() == proposal_hash) {
					let (prop_index, ..) = props.remove(index);
					if let Some((whos, amount)) = DepositOf::<T>::take(dao_id, prop_index) {
						for who in whos.into_iter() {
							match dao_token {
								DaoToken::FungibleToken(token_id) => {
									let (credit_of, _balance) =
										T::Assets::slash_held(token_id, &who, amount);

									// TODO: check
									if let Ok(()) = credit_of.try_drop() {}
								},
								DaoToken::EthTokenAddress(_) => {},
							}
						}
					}
				}
			});

			// Remove the external queued referendum, if it's there.
			if matches!(NextExternal::<T>::get(dao_id), Some((p, ..)) if p.hash() == proposal_hash)
			{
				NextExternal::<T>::remove(dao_id);
			}

			// Remove the referendum, if it's there.
			if let Some(ref_index) = maybe_ref_index {
				if let Ok(status) = Self::referendum_status(dao_id, ref_index) {
					if status.proposal.hash() == proposal_hash {
						Self::internal_cancel_referendum(dao_id, ref_index);
					}
				}
			}

			Self::deposit_event(Event::<T>::Blacklisted { dao_id, proposal_hash });
			Ok(())
		}

		/// Remove a proposal.
		///
		/// The dispatch origin of this call must be `CancelProposalOrigin`.
		///
		/// - `prop_index`: The index of the proposal to cancel.
		///
		/// Weight: `O(p)` where `p = PublicProps::<T>::decode_len()`
		#[pallet::weight(T::WeightInfo::cancel_proposal())]
		pub fn cancel_proposal(
			origin: OriginFor<T>,
			dao_id: DaoId,
			#[pallet::compact] prop_index: PropIndex,
		) -> DispatchResult {
			let dao_account_id = T::DaoProvider::dao_account_id(dao_id);

			let GovernanceV1Policy { cancel_proposal_origin, .. } =
				Self::ensure_democracy_supported(dao_id)?;

			T::CancelProposalOrigin::ensure_origin(
				origin,
				&DaoOrigin { dao_account_id, proportion: cancel_proposal_origin },
			)?;

			let dao_token = T::DaoProvider::dao_token(dao_id)?;

			PublicProps::<T>::mutate(dao_id, |props| props.retain(|p| p.0 != prop_index));
			if let Some((whos, amount)) = DepositOf::<T>::take(dao_id, prop_index) {
				for who in whos.into_iter() {
					match dao_token {
						DaoToken::FungibleToken(token_id) => {
							let (credit_of, _balance) =
								T::Assets::slash_held(token_id, &who, amount);

							// TODO: check
							if let Ok(()) = credit_of.try_drop() {}
						},
						DaoToken::EthTokenAddress(_) => {},
					}
				}
			}

			Self::deposit_event(Event::<T>::ProposalCanceled { dao_id, prop_index });
			Ok(())
		}
	}
}

pub trait EncodeInto: Encode {
	fn encode_into<T: AsMut<[u8]> + Default>(&self) -> T {
		let mut t = T::default();
		self.using_encoded(|data| {
			if data.len() <= t.as_mut().len() {
				t.as_mut()[0..data.len()].copy_from_slice(data);
			} else {
				// encoded self is too big to fit into a T. hash it and use the first bytes of that
				// instead.
				let hash = sp_io::hashing::blake2_256(data);
				let l = t.as_mut().len().min(hash.len());
				t.as_mut()[0..l].copy_from_slice(&hash[0..l]);
			}
		});
		t
	}
}
impl<T: Encode> EncodeInto for T {}

impl<T: Config> Pallet<T> {
	pub fn ensure_democracy_supported(dao_id: DaoId) -> Result<GovernanceV1Policy, DispatchError> {
		let DaoPolicy { governance, .. } = T::DaoProvider::policy(dao_id)?;
		if governance.is_none() {
			return Err(Error::<T>::NotSupported.into())
		}

		match governance.unwrap() {
			DaoGovernance::GovernanceV1(governance) => Ok(governance),
			DaoGovernance::OwnershipWeightedVoting => Err(Error::<T>::NotSupported.into()),
		}
	}

	/// Get the amount locked in support of `proposal`; `None` if proposal isn't a valid proposal
	/// index.
	pub fn backing_for(dao_id: DaoId, proposal: PropIndex) -> Option<BalanceOf<T>> {
		Self::deposit_of(dao_id, proposal).map(|(l, d)| d.saturating_mul((l.len() as u32).into()))
	}

	/// Get all referenda ready for tally at block `n`.
	pub fn maturing_referenda_at(
		dao_id: DaoId,
		n: T::BlockNumber,
	) -> Vec<(ReferendumIndex, ReferendumStatus<T::BlockNumber, BoundedCallOf<T>, BalanceOf<T>>)> {
		let next = Self::lowest_unbaked(dao_id);
		let last = Self::referendum_count(dao_id);
		Self::maturing_referenda_at_inner(dao_id, n, next..last)
	}

	fn maturing_referenda_at_inner(
		dao_id: DaoId,
		n: T::BlockNumber,
		range: core::ops::Range<PropIndex>,
	) -> Vec<(ReferendumIndex, ReferendumStatus<T::BlockNumber, BoundedCallOf<T>, BalanceOf<T>>)> {
		range
			.into_iter()
			.map(|i| (i, Self::referendum_info(dao_id, i)))
			.filter_map(|(i, maybe_info)| match maybe_info {
				Some(ReferendumInfo::Ongoing(status)) => Some((i, status)),
				_ => None,
			})
			.filter(|(_, status)| status.end == n)
			.collect()
	}

	/// Remove a referendum.
	pub fn internal_cancel_referendum(dao_id: DaoId, ref_index: ReferendumIndex) {
		Self::deposit_event(Event::<T>::Cancelled { dao_id, ref_index });
		ReferendumInfoOf::<T>::remove(dao_id, ref_index);
	}

	// private.

	/// Ok if the given referendum is active, Err otherwise
	fn ensure_ongoing(
		r: ReferendumInfo<T::BlockNumber, BoundedCallOf<T>, BalanceOf<T>>,
	) -> Result<ReferendumStatus<T::BlockNumber, BoundedCallOf<T>, BalanceOf<T>>, DispatchError> {
		match r {
			ReferendumInfo::Ongoing(s) => Ok(s),
			_ => Err(Error::<T>::ReferendumInvalid.into()),
		}
	}

	fn referendum_status(
		dao_id: DaoId,
		ref_index: ReferendumIndex,
	) -> Result<ReferendumStatus<T::BlockNumber, BoundedCallOf<T>, BalanceOf<T>>, DispatchError> {
		let info =
			ReferendumInfoOf::<T>::get(dao_id, ref_index).ok_or(Error::<T>::ReferendumInvalid)?;
		Self::ensure_ongoing(info)
	}

	/// Actually enact a vote, if legit.
	fn try_vote(
		dao_id: DaoId,
		who: &T::AccountId,
		ref_index: ReferendumIndex,
		vote: AccountVote<BalanceOf<T>>,
	) -> DispatchResult {
		let mut status = Self::referendum_status(dao_id, ref_index)?;

		let dao_token = T::DaoProvider::dao_token(dao_id)?;
		match dao_token {
			DaoToken::FungibleToken(token_id) => {
				ensure!(
					vote.balance() <= T::Assets::balance(token_id, who),
					Error::<T>::InsufficientFunds
				)
			},
			DaoToken::EthTokenAddress(_) => {},
		}

		VotingOf::<T>::try_mutate(dao_id, who, |voting| -> DispatchResult {
			if let Voting::Direct { ref mut votes, delegations, .. } = voting {
				match votes.binary_search_by_key(&ref_index, |i| i.0) {
					Ok(i) => {
						// Shouldn't be possible to fail, but we handle it gracefully.
						status.tally.remove(votes[i].1).ok_or(ArithmeticError::Underflow)?;
						if let Some(approve) = votes[i].1.as_standard() {
							status.tally.reduce(approve, *delegations);
						}
						votes[i].1 = vote;
					},
					Err(i) => {
						votes
							.try_insert(i, (ref_index, vote))
							.map_err(|_| Error::<T>::MaxVotesReached)?;
					},
				}
				Self::deposit_event(Event::<T>::Voted {
					dao_id,
					voter: who.clone(),
					ref_index,
					vote,
				});
				// Shouldn't be possible to fail, but we handle it gracefully.
				status.tally.add(vote).ok_or(ArithmeticError::Overflow)?;
				if let Some(approve) = vote.as_standard() {
					status.tally.increase(approve, *delegations);
				}
				Ok(())
			} else {
				Err(Error::<T>::AlreadyDelegating.into())
			}
		})?;
		// Extend the lock to `balance` (rather than setting it) since we don't know what other
		// votes are in place.
		match dao_token {
			DaoToken::FungibleToken(token_id) =>
				T::Assets::extend_lock(DEMOCRACY_ID, &token_id, who, vote.balance()),
			DaoToken::EthTokenAddress(_) => {},
		}

		ReferendumInfoOf::<T>::insert(dao_id, ref_index, ReferendumInfo::Ongoing(status));
		Ok(())
	}

	/// Remove the account's vote for the given referendum if possible. This is possible when:
	/// - The referendum has not finished.
	/// - The referendum has finished and the voter lost their direction.
	/// - The referendum has finished and the voter's lock period is up.
	///
	/// This will generally be combined with a call to `unlock`.
	fn try_remove_vote(
		dao_id: DaoId,
		who: &T::AccountId,
		ref_index: ReferendumIndex,
		scope: UnvoteScope,
	) -> DispatchResult {
		let GovernanceV1Policy { vote_locking_period, .. } =
			Self::ensure_democracy_supported(dao_id)?;

		let info = ReferendumInfoOf::<T>::get(dao_id, ref_index);
		VotingOf::<T>::try_mutate(dao_id, who, |voting| -> DispatchResult {
			if let Voting::Direct { ref mut votes, delegations, ref mut prior } = voting {
				let i = votes
					.binary_search_by_key(&ref_index, |i| i.0)
					.map_err(|_| Error::<T>::NotVoter)?;
				match info {
					Some(ReferendumInfo::Ongoing(mut status)) => {
						ensure!(matches!(scope, UnvoteScope::Any), Error::<T>::NoPermission);
						// Shouldn't be possible to fail, but we handle it gracefully.
						status.tally.remove(votes[i].1).ok_or(ArithmeticError::Underflow)?;
						if let Some(approve) = votes[i].1.as_standard() {
							status.tally.reduce(approve, *delegations);
						}
						ReferendumInfoOf::<T>::insert(
							dao_id,
							ref_index,
							ReferendumInfo::Ongoing(status),
						);
					},
					Some(ReferendumInfo::Finished { end, approved }) => {
						if let Some((lock_periods, balance)) = votes[i].1.locked_if(approved) {
							let unlock_at = end.saturating_add(
								Self::u32_to_block_number(vote_locking_period)
									.saturating_mul(lock_periods.into()),
							);
							let now = frame_system::Pallet::<T>::block_number();
							if now < unlock_at {
								ensure!(
									matches!(scope, UnvoteScope::Any),
									Error::<T>::NoPermission
								);
								prior.accumulate(unlock_at, balance)
							}
						}
					},
					None => {}, // Referendum was cancelled.
				}
				votes.remove(i);
			}
			Ok(())
		})?;
		Ok(())
	}

	/// Return the number of votes for `who`
	fn increase_upstream_delegation(
		dao_id: DaoId,
		who: &T::AccountId,
		amount: Delegations<BalanceOf<T>>,
	) -> u32 {
		VotingOf::<T>::mutate(dao_id, who, |voting| match voting {
			Voting::Delegating { delegations, .. } => {
				// We don't support second level delegating, so we don't need to do anything more.
				*delegations = delegations.saturating_add(amount);
				1
			},
			Voting::Direct { votes, delegations, .. } => {
				*delegations = delegations.saturating_add(amount);
				for &(ref_index, account_vote) in votes.iter() {
					if let AccountVote::Standard { vote, .. } = account_vote {
						ReferendumInfoOf::<T>::mutate(dao_id, ref_index, |maybe_info| {
							if let Some(ReferendumInfo::Ongoing(ref mut status)) = maybe_info {
								status.tally.increase(vote.aye, amount);
							}
						});
					}
				}
				votes.len() as u32
			},
		})
	}

	/// Return the number of votes for `who`
	fn reduce_upstream_delegation(
		dao_id: DaoId,
		who: &T::AccountId,
		amount: Delegations<BalanceOf<T>>,
	) -> u32 {
		VotingOf::<T>::mutate(dao_id, who, |voting| match voting {
			Voting::Delegating { delegations, .. } => {
				// We don't support second level delegating, so we don't need to do anything more.
				*delegations = delegations.saturating_sub(amount);
				1
			},
			Voting::Direct { votes, delegations, .. } => {
				*delegations = delegations.saturating_sub(amount);
				for &(ref_index, account_vote) in votes.iter() {
					if let AccountVote::Standard { vote, .. } = account_vote {
						ReferendumInfoOf::<T>::mutate(dao_id, ref_index, |maybe_info| {
							if let Some(ReferendumInfo::Ongoing(ref mut status)) = maybe_info {
								status.tally.reduce(vote.aye, amount);
							}
						});
					}
				}
				votes.len() as u32
			},
		})
	}

	/// Attempt to delegate `balance` times `conviction` of voting power from `who` to `target`.
	///
	/// Return the upstream number of votes.
	fn try_delegate(
		dao_id: DaoId,
		who: T::AccountId,
		target: T::AccountId,
		conviction: Conviction,
		balance: BalanceOf<T>,
	) -> Result<u32, DispatchError> {
		ensure!(who != target, Error::<T>::Nonsense);

		let dao_token = T::DaoProvider::dao_token(dao_id)?;
		match dao_token {
			DaoToken::FungibleToken(token_id) => {
				ensure!(
					balance <= T::Assets::balance(token_id, &who),
					Error::<T>::InsufficientFunds
				);
			},
			DaoToken::EthTokenAddress(_) => {},
		}

		let GovernanceV1Policy { vote_locking_period, .. } =
			Self::ensure_democracy_supported(dao_id)?;

		let votes =
			VotingOf::<T>::try_mutate(dao_id, &who, |voting| -> Result<u32, DispatchError> {
				let mut old = Voting::Delegating {
					balance,
					target: target.clone(),
					conviction,
					delegations: Default::default(),
					prior: Default::default(),
				};
				sp_std::mem::swap(&mut old, voting);
				match old {
					Voting::Delegating {
						balance,
						target,
						conviction,
						delegations,
						mut prior,
						..
					} => {
						// remove any delegation votes to our current target.
						Self::reduce_upstream_delegation(
							dao_id,
							&target,
							conviction.votes(balance),
						);
						let now = frame_system::Pallet::<T>::block_number();
						let lock_periods = conviction.lock_periods().into();
						let unlock_block = now.saturating_add(
							Self::u32_to_block_number(vote_locking_period)
								.saturating_mul(lock_periods),
						);
						prior.accumulate(unlock_block, balance);
						voting.set_common(delegations, prior);
					},
					Voting::Direct { votes, delegations, prior } => {
						// here we just ensure that we're currently idling with no votes recorded.
						ensure!(votes.is_empty(), Error::<T>::VotesExist);
						voting.set_common(delegations, prior);
					},
				}
				let votes =
					Self::increase_upstream_delegation(dao_id, &target, conviction.votes(balance));
				// Extend the lock to `balance` (rather than setting it) since we don't know what
				// other votes are in place.
				match dao_token {
					DaoToken::FungibleToken(token_id) =>
						T::Assets::extend_lock(DEMOCRACY_ID, &token_id, &who, balance),
					DaoToken::EthTokenAddress(_) => {},
				}
				Ok(votes)
			})?;
		Self::deposit_event(Event::<T>::Delegated { dao_id, who, target });
		Ok(votes)
	}

	// TODO: optimize DAO policy retrieval
	/// Attempt to end the current delegation.
	///
	/// Return the number of votes of upstream.
	fn try_undelegate(dao_id: DaoId, who: T::AccountId) -> Result<u32, DispatchError> {
		let GovernanceV1Policy { vote_locking_period, .. } =
			Self::ensure_democracy_supported(dao_id)?;

		let votes =
			VotingOf::<T>::try_mutate(dao_id, &who, |voting| -> Result<u32, DispatchError> {
				let mut old = Voting::default();
				sp_std::mem::swap(&mut old, voting);
				match old {
					Voting::Delegating { balance, target, conviction, delegations, mut prior } => {
						// remove any delegation votes to our current target.
						let votes = Self::reduce_upstream_delegation(
							dao_id,
							&target,
							conviction.votes(balance),
						);
						let now = frame_system::Pallet::<T>::block_number();
						let lock_periods = conviction.lock_periods().into();
						let unlock_block = now.saturating_add(
							Self::u32_to_block_number(vote_locking_period)
								.saturating_mul(lock_periods),
						);
						prior.accumulate(unlock_block, balance);
						voting.set_common(delegations, prior);

						Ok(votes)
					},
					Voting::Direct { .. } => Err(Error::<T>::NotDelegating.into()),
				}
			})?;
		Self::deposit_event(Event::<T>::Undelegated { dao_id, account: who });
		Ok(votes)
	}

	/// Rejig the lock on an account. It will never get more stringent (since that would indicate
	/// a security hole) but may be reduced from what they are currently.
	fn update_lock(dao_id: DaoId, who: &T::AccountId, dao_token: DaoToken<u32, Vec<u8>>) {
		let lock_needed = VotingOf::<T>::mutate(dao_id, who, |voting| {
			voting.rejig(frame_system::Pallet::<T>::block_number());
			voting.locked_balance()
		});

		match dao_token {
			DaoToken::FungibleToken(token_id) =>
				if lock_needed.is_zero() {
					T::Assets::remove_lock(DEMOCRACY_ID, &token_id, who);
				} else {
					T::Assets::set_lock(DEMOCRACY_ID, &token_id, who, lock_needed);
				},
			DaoToken::EthTokenAddress(_) => {},
		}
	}

	/// Start a referendum
	fn inject_referendum(
		dao_id: DaoId,
		end: T::BlockNumber,
		proposal: BoundedCallOf<T>,
		threshold: VoteThreshold,
		delay: T::BlockNumber,
	) -> ReferendumIndex {
		let ref_index = Self::referendum_count(dao_id);
		ReferendumCount::<T>::insert(dao_id, ref_index + 1);
		let status =
			ReferendumStatus { end, proposal, threshold, delay, tally: Default::default() };
		let item = ReferendumInfo::Ongoing(status);
		<ReferendumInfoOf<T>>::insert(dao_id, ref_index, item);
		Self::deposit_event(Event::<T>::Started { dao_id, ref_index, threshold });
		ref_index
	}

	/// Table the next waiting proposal for a vote.
	fn launch_next(dao_id: DaoId, now: T::BlockNumber) -> DispatchResult {
		if LastTabledWasExternal::<T>::take(dao_id) {
			Self::launch_public(dao_id, now).or_else(|_| Self::launch_external(dao_id, now))
		} else {
			Self::launch_external(dao_id, now).or_else(|_| Self::launch_public(dao_id, now))
		}
		.map_err(|_| Error::<T>::NoneWaiting.into())
	}

	/// Table the waiting external proposal for a vote, if there is one.
	fn launch_external(dao_id: DaoId, now: T::BlockNumber) -> DispatchResult {
		let GovernanceV1Policy { voting_period, enactment_period, .. } =
			Self::ensure_democracy_supported(dao_id)?;

		if let Some((proposal, threshold)) = <NextExternal<T>>::take(dao_id) {
			LastTabledWasExternal::<T>::insert(dao_id, true);
			Self::deposit_event(Event::<T>::ExternalTabled);
			Self::inject_referendum(
				dao_id,
				now.saturating_add(Self::u32_to_block_number(voting_period)),
				proposal,
				threshold,
				Self::u32_to_block_number(enactment_period),
			);
			Ok(())
		} else {
			Err(Error::<T>::NoneWaiting.into())
		}
	}

	/// Table the waiting public proposal with the highest backing for a vote.
	fn launch_public(dao_id: DaoId, now: T::BlockNumber) -> DispatchResult {
		let GovernanceV1Policy { voting_period, enactment_period, .. } =
			Self::ensure_democracy_supported(dao_id)?;

		let mut public_props = Self::public_props(dao_id);
		if let Some((winner_index, _)) = public_props.iter().enumerate().max_by_key(
			// defensive only: All current public proposals have an amount locked
			|x| Self::backing_for(dao_id, (x.1).0).defensive_unwrap_or_else(Zero::zero),
		) {
			let (prop_index, proposal, _) = public_props.swap_remove(winner_index);
			<PublicProps<T>>::insert(dao_id, public_props);

			let dao_token = T::DaoProvider::dao_token(dao_id)?;

			if let Some((depositors, deposit)) = <DepositOf<T>>::take(dao_id, prop_index) {
				// refund depositors
				for d in depositors.iter() {
					match dao_token {
						DaoToken::FungibleToken(token_id) =>
							T::Assets::release(token_id, d, deposit, false)?,
						DaoToken::EthTokenAddress(_) => continue,
					};
				}
				Self::deposit_event(Event::<T>::Tabled {
					dao_id,
					proposal_index: prop_index,
					deposit,
				});
				Self::inject_referendum(
					dao_id,
					now.saturating_add(Self::u32_to_block_number(voting_period)),
					proposal,
					VoteThreshold::SuperMajorityApprove,
					Self::u32_to_block_number(enactment_period),
				);
			}
			Ok(())
		} else {
			Err(Error::<T>::NoneWaiting.into())
		}
	}

	fn bake_referendum(
		dao_id: DaoId,
		now: T::BlockNumber,
		index: ReferendumIndex,
		status: ReferendumStatus<T::BlockNumber, BoundedCallOf<T>, BalanceOf<T>>,
	) -> bool {
		let dao_token = T::DaoProvider::dao_token(dao_id).unwrap();

		let token_id = match dao_token {
			DaoToken::FungibleToken(token_id) => Some(token_id),
			DaoToken::EthTokenAddress(_) => None,
		};

		let total_issuance = match token_id {
			None => Zero::zero(),
			Some(token_id) => T::Assets::total_issuance(token_id),
		};
		let approved = status.threshold.approved(status.tally, total_issuance);

		if approved {
			let dao_account_id = T::DaoProvider::dao_account_id(dao_id);
			let origin = RawOrigin::Dao(dao_account_id).into();

			Self::deposit_event(Event::<T>::Passed { dao_id, ref_index: index });
			// Actually `hold` the proposal now since we didn't hold it when it came in via the
			// submit extrinsic and we now know that it will be needed. This will be reversed by
			// Scheduler pallet once it is executed which assumes that we will already have placed
			// a `hold` on it.

			// Earliest it can be scheduled for is next block.
			let when = now.saturating_add(status.delay.max(One::one()));
			if T::Scheduler::schedule_named(
				(DEMOCRACY_ID, index).encode_into(),
				DispatchTime::At(when),
				None,
				63,
				origin,
				status.proposal,
			)
			.is_err()
			{
				frame_support::print("LOGIC ERROR: bake_referendum/schedule_named failed");
			}
		} else {
			Self::deposit_event(Event::<T>::NotPassed { dao_id, ref_index: index });
		}

		approved
	}

	// TODO: beware of huge DAO count - use chunk spend instead
	/// Current era is ending; we should finish up any proposals.
	///
	///
	/// # <weight>
	/// If a referendum is launched or maturing, this will take full block weight if queue is not
	/// empty. Otherwise:
	/// - Complexity: `O(R)` where `R` is the number of unbaked referenda.
	/// - Db reads: `LastTabledWasExternal`, `NextExternal`, `PublicProps`, `account`,
	///   `ReferendumCount`, `LowestUnbaked`
	/// - Db writes: `PublicProps`, `account`, `ReferendumCount`, `DepositOf`, `ReferendumInfoOf`
	/// - Db reads per R: `DepositOf`, `ReferendumInfoOf`
	/// # </weight>
	fn begin_block(now: T::BlockNumber) -> Weight {
		let max_block_weight = T::BlockWeights::get().max_block;
		let mut weight = Weight::zero();

		let dao_count = T::DaoProvider::count();

		for dao_id in 0..dao_count {
			let maybe_policy = T::DaoProvider::policy(dao_id);

			// TODO
			if maybe_policy.is_err() {
				continue
			}

			let governance = Self::ensure_democracy_supported(dao_id);

			// TODO
			if governance.is_err() {
				continue
			}

			let GovernanceV1Policy { launch_period, .. } = governance.unwrap();

			let next = Self::lowest_unbaked(dao_id);
			let last = Self::referendum_count(dao_id);
			let r = last.saturating_sub(next);

			// pick out another public referendum if it's time.
			if (now % Self::u32_to_block_number(launch_period)).is_zero() {
				// Errors come from the queue being empty. If the queue is not empty, it will take
				// full block weight.
				if Self::launch_next(dao_id, now).is_ok() {
					weight = max_block_weight;
				} else {
					weight
						.saturating_accrue(T::WeightInfo::on_initialize_base_with_launch_period(r));
				}
			} else {
				weight.saturating_accrue(T::WeightInfo::on_initialize_base(r));
			}

			// tally up votes for any expiring referenda.
			for (index, info) in
				Self::maturing_referenda_at_inner(dao_id, now, next..last).into_iter()
			{
				let approved = Self::bake_referendum(dao_id, now, index, info);
				ReferendumInfoOf::<T>::insert(
					dao_id,
					index,
					ReferendumInfo::Finished { end: now, approved },
				);
				weight = max_block_weight;
			}

			// Notes:
			// * We don't consider the lowest unbaked to be the last maturing in case some referenda
			//   have a longer voting period than others.
			// * The iteration here shouldn't trigger any storage read that are not in cache, due to
			//   `maturing_referenda_at_inner` having already read them.
			// * We shouldn't iterate more than `LaunchPeriod/VotingPeriod + 1` times because the
			//   number of unbaked referendum is bounded by this number. In case those number have
			//   changed in a runtime upgrade the formula should be adjusted but the bound should
			//   still be sensible.
			<LowestUnbaked<T>>::mutate(dao_id, |ref_index| {
				while *ref_index < last &&
					Self::referendum_info(dao_id, *ref_index)
						.map_or(true, |info| matches!(info, ReferendumInfo::Finished { .. }))
				{
					*ref_index += 1
				}
			});
		}

		weight
	}

	/// Reads the length of account in DepositOf without getting the complete value in the runtime.
	///
	/// Return 0 if no deposit for this proposal.
	fn len_of_deposit_of(dao_id: DaoId, proposal: PropIndex) -> Option<u32> {
		// DepositOf first tuple element is a vec, decoding its len is equivalent to decode a
		// `Compact<u32>`.
		decode_compact_u32_at(&<DepositOf<T>>::hashed_key_for(dao_id, proposal))
	}

	fn u128_to_balance_of(cost: u128) -> BalanceOf<T> {
		TryInto::<BalanceOf<T>>::try_into(cost).ok().unwrap()
	}

	fn u32_to_block_number(block: u32) -> <T as frame_system::Config>::BlockNumber {
		TryInto::<<T as frame_system::Config>::BlockNumber>::try_into(block)
			.ok()
			.unwrap()
	}
}

/// Decode `Compact<u32>` from the trie at given key.
fn decode_compact_u32_at(key: &[u8]) -> Option<u32> {
	// `Compact<u32>` takes at most 5 bytes.
	let mut buf = [0u8; 5];
	let bytes = sp_io::storage::read(key, &mut buf, 0)?;
	// The value may be smaller than 5 bytes.
	let mut input = &buf[0..buf.len().min(bytes as usize)];
	match codec::Compact::<u32>::decode(&mut input) {
		Ok(c) => Some(c.0),
		Err(_) => {
			sp_runtime::print("Failed to decode compact u32 at:");
			sp_runtime::print(key);
			None
		},
	}
}
