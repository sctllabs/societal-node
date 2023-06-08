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

// Re-purposing the pallet to manage collective for the DAOs created by the pallet-dao factory:
// - re-worked pallet storage to persist collective data for each DAO
// - updated pallet extrinsic functions adding dao support
// - added support for DaoProvider retrieving custom configuration for each DAO
// - updated origins using EnsureOriginWithArg to support custom configuration by DaoProvider
// - removed GenesisConfig
// - removed support for 'prime' member

//! Collective system: Members of a set of account IDs can make their collective feelings known
//! through dispatched calls from one of two specialized origins.
//!
//! The membership can be provided through implementing the `ChangeMembers`.
//! The pallet assumes that the amount of members stays at or below `MaxMembers` for its weight
//! calculations, but enforces this in `change_members_sorted`.
//!
//! Voting happens through motions comprising a proposal (i.e. a curried dispatchable) plus a
//! number of approvals required for it to pass and be called. Motions are open for members to
//! vote on for a minimum period given by `MotionDuration`. As soon as the needed number of
//! approvals is given, the motion is closed and executed. If the number of approvals is not reached
//! during the voting period, then `close` may be called by any account in order to force the end
//! the motion explicitly. The proposal is executed if there are enough approvals counting the new
//! votes.
//!
//! If there are not, then the motion is dropped without being executed.

#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "128"]

use scale_info::TypeInfo;
use sp_io::storage;
use sp_runtime::{traits::Hash, Either, Permill, RuntimeDebug, Saturating};
use sp_std::{marker::PhantomData, prelude::*, result, str};

use dao_primitives::{
	ChangeDaoMembers, DaoOrigin, DaoPolicy, DaoPolicyProportion, DaoProvider, EncodeInto,
	InitializeDaoMembers, RawOrigin as DaoRawOrigin,
};

use frame_support::{
	codec::{Decode, Encode, MaxEncodedLen},
	dispatch::{
		DispatchError, DispatchResultWithPostInfo, Dispatchable, GetDispatchInfo, Pays,
		PostDispatchInfo,
	},
	ensure,
	traits::{
		schedule::{
			v3::{Named as ScheduleNamed, TaskName},
			DispatchTime,
		},
		Backing, Bounded, EnsureOrigin, EnsureOriginWithArg, Get, GetBacking, LockIdentifier,
		QueryPreimage, StorageVersion, StorePreimage,
	},
	weights::Weight,
	Parameter,
};
use sp_core::bounded::BoundedVec;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;

pub use pallet::*;
pub use weights::WeightInfo;

/// Dao ID. Just a `u32`.
pub type DaoId = u32;

/// Simple index type for proposal counting.
pub type ProposalIndex = u32;

/// A number of members.
///
/// This also serves as a number of voting members, and since for motions, each member may
/// vote exactly once, therefore also the number of votes for any given motion.
pub type MemberCount = u32;

pub type BoundedProposal<T, I> = Bounded<<T as Config<I>>::Proposal>;

pub type CallOf<T, I> = <T as Config<I>>::RuntimeCall;

const DAO_COLLECTIVE_ID: LockIdentifier = *b"daoclctv";

/// Default voting strategy when a member is inactive.
pub trait DefaultVote {
	/// Get the default voting strategy, given:
	///
	/// - Raw number of yes votes.
	/// - Raw number of no votes.
	/// - Total number of member count.
	fn default_vote(yes_votes: MemberCount, no_votes: MemberCount, len: MemberCount) -> bool;
}

/// set the default vote as yes if yes vote are over majority of the whole collective.
pub struct MoreThanMajorityVote;
impl DefaultVote for MoreThanMajorityVote {
	fn default_vote(yes_votes: MemberCount, _no_votes: MemberCount, len: MemberCount) -> bool {
		yes_votes * 2 > len
	}
}

/// Origin for the collective module.
#[derive(PartialEq, Eq, Clone, RuntimeDebug, Encode, Decode, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(I))]
#[codec(mel_bound(AccountId: MaxEncodedLen))]
pub enum RawOrigin<AccountId, I> {
	/// It has been condoned by a given number of members of the collective from a given total.
	Members(MemberCount, MemberCount),
	/// It has been condoned by a single member of the collective.
	Member(AccountId),
	/// Dummy to manage the fact we have instancing.
	_Phantom(PhantomData<I>),
}

impl<AccountId, I> GetBacking for RawOrigin<AccountId, I> {
	fn get_backing(&self) -> Option<Backing> {
		match self {
			RawOrigin::Members(n, d) => Some(Backing { approvals: *n, eligible: *d }),
			_ => None,
		}
	}
}

/// Info for keeping track of a motion being voted on.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct Votes<BlockNumber, VotingSet> {
	/// The proposal's unique index.
	index: ProposalIndex,
	/// The number of approval votes that are needed to pass the motion.
	threshold: MemberCount,
	/// The current set of voters that approved it.
	ayes: VotingSet,
	/// The current set of voters that rejected it.
	nays: VotingSet,
	/// The hard end time of this vote.
	end: BlockNumber,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		pallet_prelude::*,
		traits::{QueryPreimage, StorePreimage},
	};
	use frame_system::pallet_prelude::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(4);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		type RuntimeCall: Parameter
			+ Dispatchable<RuntimeOrigin = <Self as Config<I>>::RuntimeOrigin>
			+ From<Call<Self, I>>
			+ IsType<<Self as frame_system::Config>::RuntimeCall>
			+ From<frame_system::Call<Self>>;

		/// The outer origin type.
		type RuntimeOrigin: From<RawOrigin<Self::AccountId, I>>;

		/// The outer call dispatch type.
		type Proposal: Parameter
			+ Dispatchable<
				RuntimeOrigin = <Self as Config<I>>::RuntimeOrigin,
				PostInfo = PostDispatchInfo,
			> + From<frame_system::Call<Self>>
			+ GetDispatchInfo;

		/// The outer event type.
		type RuntimeEvent: From<Event<Self, I>>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;

		#[pallet::constant]
		type ProposalMetadataLimit: Get<u32>;

		/// Maximum number of proposals allowed to be active in parallel.
		type MaxProposals: Get<ProposalIndex>;

		/// The maximum number of members supported by the pallet. Used for weight estimation.
		///
		/// NOTE:
		/// + Benchmarks will need to be re-run and weights adjusted if this changes.
		/// + This pallet assumes that dependents keep to the limit without enforcing it.
		type MaxMembers: Get<MemberCount>;

		/// The maximum number of votes(ayes/nays) for a proposal.
		#[pallet::constant]
		type MaxVotes: Get<u32>;

		/// Default vote strategy of this collective.
		type DefaultVote: DefaultVote;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;

		type DaoProvider: DaoProvider<
			<Self as frame_system::Config>::Hash,
			Id = u32,
			AccountId = Self::AccountId,
			Policy = DaoPolicy,
			Origin = OriginFor<Self>,
		>;

		/// The preimage provider with which we look up call hashes to get the call.
		type Preimages: QueryPreimage + StorePreimage;

		type Scheduler: ScheduleNamed<Self::BlockNumber, CallOf<Self, I>, Self::PalletsOrigin>;

		/// Overarching type of all pallets origins.
		type PalletsOrigin: From<DaoRawOrigin<Self::AccountId>>;
	}

	/// Origin for the collective pallet.
	#[pallet::origin]
	pub type Origin<T, I = ()> = RawOrigin<<T as frame_system::Config>::AccountId, I>;

	/// The hashes of the active proposals by Dao.
	#[pallet::storage]
	#[pallet::getter(fn proposals)]
	pub type Proposals<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Twox64Concat, DaoId, BoundedVec<T::Hash, T::MaxProposals>, ValueQuery>;

	/// Actual proposal for a given hash, if it's current.
	#[pallet::storage]
	#[pallet::getter(fn proposal_of)]
	pub type ProposalOf<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
		_,
		Twox64Concat,
		DaoId,
		Identity,
		T::Hash,
		BoundedProposal<T, I>,
		OptionQuery,
	>;

	/// Votes on a given proposal, if it is ongoing.
	#[pallet::storage]
	#[pallet::getter(fn voting)]
	pub type Voting<T: Config<I>, I: 'static = ()> = StorageDoubleMap<
		_,
		Twox64Concat,
		DaoId,
		Identity,
		T::Hash,
		Votes<T::BlockNumber, BoundedVec<T::AccountId, T::MaxVotes>>,
		OptionQuery,
	>;

	/// Proposals so far.
	#[pallet::storage]
	#[pallet::getter(fn proposal_count)]
	pub type ProposalCount<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Twox64Concat, DaoId, u32, ValueQuery>;

	/// The current members of the Dao collective. This is stored sorted (just by value).
	#[pallet::storage]
	#[pallet::getter(fn members)]
	pub type Members<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Twox64Concat, DaoId, BoundedVec<T::AccountId, T::MaxMembers>, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		/// A motion (given hash) has been proposed (by given account) with a threshold (given
		/// `MemberCount`).
		Proposed {
			dao_id: DaoId,
			account: T::AccountId,
			proposal_index: ProposalIndex,
			proposal_hash: T::Hash,
			proposal: T::Proposal,
			threshold: MemberCount,
			meta: Option<Vec<u8>>,
		},
		/// A motion (given hash) has been voted on by given account, leaving
		/// a tally (yes votes and no votes given respectively as `MemberCount`).
		Voted {
			dao_id: DaoId,
			account: T::AccountId,
			proposal_index: ProposalIndex,
			proposal_hash: T::Hash,
			voted: bool,
			yes: MemberCount,
			no: MemberCount,
		},
		/// A motion was approved by the required threshold.
		Approved { dao_id: DaoId, proposal_index: ProposalIndex, proposal_hash: T::Hash },
		/// A motion was not approved by the required threshold.
		Disapproved { dao_id: DaoId, proposal_index: ProposalIndex, proposal_hash: T::Hash },
		/// A motion was executed; result will be `Ok` if it returned without error.
		Executed {
			dao_id: DaoId,
			proposal_index: ProposalIndex,
			proposal_hash: T::Hash,
			result: DispatchResult,
		},
		/// A proposal was closed because its threshold was reached or after its duration was up.
		Closed {
			dao_id: DaoId,
			proposal_index: ProposalIndex,
			proposal_hash: T::Hash,
			yes: MemberCount,
			no: MemberCount,
		},
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// Account is not a member
		NotMember,
		/// Duplicate proposals not allowed
		DuplicateProposal,
		/// Proposal must exist
		ProposalMissing,
		/// Mismatched index
		WrongIndex,
		/// Duplicate vote ignored
		DuplicateVote,
		/// Members are already initialized!
		AlreadyInitialized,
		/// The close call was made too early, before the end of the voting.
		TooEarly,
		/// There can only be a maximum of `MaxProposals` active proposals.
		TooManyProposals,
		/// The given weight bound for the proposal was too low.
		WrongProposalWeight,
		/// The given length bound for the proposal was too low.
		WrongProposalLength,
		/// Metadata size exceeds the limits
		MetadataTooLong,
		/// There can only be a maximum of `MaxVotes` votes for proposal.
		TooManyVotes,
		/// There can only be a maximum of `MaxMembers` votes for proposal.
		TooManyMembers,
		/// Voting is disabled for expired proposals
		Expired,
	}

	// Note that councillor operations are assigned to the operational class.
	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		/// Add a new proposal to either be voted on or executed directly.
		///
		/// Requires the sender to be member.
		///
		/// `threshold` determines whether `proposal` is executed directly (`threshold < 2`)
		/// or put up for voting.
		#[pallet::weight((
			T::WeightInfo::propose_with_meta(
				*length_bound,
				T::MaxMembers::get(),
				T::MaxProposals::get()
			),
			DispatchClass::Operational
		))]
		#[pallet::call_index(1)]
		pub fn propose(
			origin: OriginFor<T>,
			dao_id: DaoId,
			proposal: Box<<T as Config<I>>::Proposal>,
			// TODO: remove since bounded is used
			#[pallet::compact] length_bound: u32,
		) -> DispatchResultWithPostInfo {
			Self::propose_with_meta(origin, dao_id, proposal, length_bound, None)
		}

		/// Adds a new proposal with temporary meta field for arbitrary data indexed by node indexer
		#[pallet::weight((
			T::WeightInfo::propose_with_meta(
				*length_bound,
				T::MaxMembers::get(),
				T::MaxProposals::get()
			),
			DispatchClass::Operational
		))]
		#[pallet::call_index(2)]
		pub fn propose_with_meta(
			origin: OriginFor<T>,
			dao_id: DaoId,
			proposal: Box<<T as Config<I>>::Proposal>,
			#[pallet::compact] length_bound: u32,
			meta: Option<Vec<u8>>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let members = Self::members(dao_id);
			ensure!(members.contains(&who), Error::<T, I>::NotMember);

			let proposal = T::Preimages::bound(proposal)?.transmute();

			if let Some(metadata) = meta.clone() {
				ensure!(
					BoundedVec::<u8, <T as Config<I>>::ProposalMetadataLimit>::try_from(metadata)
						.is_ok(),
					Error::<T, I>::MetadataTooLong
				)
			}

			let (proposal_len, active_proposals) =
				Self::do_propose_proposed(who, dao_id, proposal, length_bound, meta)?;

			Ok(Some(T::WeightInfo::propose_with_meta(
				proposal_len,         // B
				members.len() as u32, // M
				active_proposals,     // P2
			))
			.into())
		}

		/// Add an aye or nay vote for the sender to the given proposal.
		///
		/// Requires the sender to be a member.
		///
		/// Transaction fees will be waived if the member is voting on any particular proposal
		/// for the first time and the call is successful. Subsequent vote changes will charge a
		/// fee.
		#[pallet::weight((
			T::WeightInfo::vote(T::MaxMembers::get()),
			DispatchClass::Operational
		))]
		#[pallet::call_index(3)]
		pub fn vote(
			origin: OriginFor<T>,
			dao_id: DaoId,
			proposal: T::Hash,
			#[pallet::compact] index: ProposalIndex,
			approve: bool,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let members = Self::members(dao_id);
			ensure!(members.contains(&who), Error::<T, I>::NotMember);

			// Detects first vote of the member in the motion
			let is_account_voting_first_time =
				Self::do_vote(who, dao_id, proposal, index, approve)?;

			if is_account_voting_first_time {
				Ok((Some(T::WeightInfo::vote(members.len() as u32)), Pays::No).into())
			} else {
				Ok((Some(T::WeightInfo::vote(members.len() as u32)), Pays::Yes).into())
			}
		}

		/// Close a vote that is either approved, disapproved or whose voting period has ended.
		///
		/// May be called by any signed account in order to finish voting and close the proposal.
		///
		/// If called before the end of the voting period it will only close the vote if it is
		/// has enough votes to be approved or disapproved.
		///
		/// If called after the end of the voting period abstentions are counted as rejections
		///
		/// If the close operation completes successfully with disapproval, the transaction fee will
		/// be waived. Otherwise execution of the approved operation will be charged to the caller.
		///
		/// + `proposal_weight_bound`: The maximum amount of weight consumed by executing the closed
		/// proposal.
		/// proposal.
		/// + `length_bound`: The upper bound for the length of the proposal in storage. Checked via
		/// `storage::read` so it is `size_of::<u32>() == 4` larger than the pure length.
		#[pallet::weight((
			{
				let b = *length_bound;
				let m = T::MaxMembers::get();
				let p1 = *proposal_weight_bound;
				let p2 = T::MaxProposals::get();
				T::WeightInfo::close_early_approved(b, m, p2)
					.max(T::WeightInfo::close_early_disapproved(m, p2))
					.max(T::WeightInfo::close_approved(b, m, p2))
					.max(T::WeightInfo::close_disapproved(m, p2))
					.saturating_add(p1.into())
			},
			DispatchClass::Operational
		))]
		#[pallet::call_index(4)]
		pub fn close(
			origin: OriginFor<T>,
			dao_id: DaoId,
			proposal_hash: T::Hash,
			#[pallet::compact] index: ProposalIndex,
			proposal_weight_bound: Weight,
			#[pallet::compact] length_bound: u32,
		) -> DispatchResultWithPostInfo {
			let _ = ensure_signed(origin)?;

			match Self::do_close(dao_id, proposal_hash, index, proposal_weight_bound, length_bound)
			{
				Ok(post_info) => {
					// Canceling previously scheduled proposal close task
					T::Scheduler::cancel_named(Self::close_proposal_task_id(
						dao_id,
						proposal_hash,
						index,
					))?;

					Ok(post_info)
				},
				Err(e) => Err(e),
			}
		}

		#[pallet::weight((
			{
				let b = *length_bound;
				let m = T::MaxMembers::get();
				let p1 = *proposal_weight_bound;
				let p2 = T::MaxProposals::get();
				T::WeightInfo::close_early_approved(b, m, p2)
					.max(T::WeightInfo::close_early_disapproved(m, p2))
					.max(T::WeightInfo::close_approved(b, m, p2))
					.max(T::WeightInfo::close_disapproved(m, p2))
					.saturating_add(p1.into())
			},
			DispatchClass::Operational
		))]
		#[pallet::call_index(5)]
		pub fn close_scheduled_proposal(
			origin: OriginFor<T>,
			dao_id: DaoId,
			proposal_hash: T::Hash,
			#[pallet::compact] index: ProposalIndex,
			proposal_weight_bound: Weight,
			#[pallet::compact] length_bound: u32,
		) -> DispatchResultWithPostInfo {
			T::DaoProvider::ensure_approved(origin, dao_id)?;

			Self::do_close(dao_id, proposal_hash, index, proposal_weight_bound, length_bound)
		}
	}
}

/// Return the weight of a dispatch call result as an `Option`.
///
/// Will return the weight regardless of what the state of the result is.
fn get_result_weight(result: DispatchResultWithPostInfo) -> Option<Weight> {
	match result {
		Ok(post_info) => post_info.actual_weight,
		Err(err) => err.post_info.actual_weight,
	}
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
	/// Check whether `who` is a member of the collective.
	pub fn is_member(dao_id: DaoId, who: &T::AccountId) -> bool {
		// Note: The dispatchables *do not* use this to check membership so make sure
		// to update those if this is changed.
		Self::members(dao_id).contains(who)
	}

	/// Add a new proposal to be voted.
	pub fn do_propose_proposed(
		who: T::AccountId,
		dao_id: DaoId,
		proposal: BoundedProposal<T, I>,
		length_bound: MemberCount,
		meta: Option<Vec<u8>>,
	) -> Result<(u32, u32), DispatchError> {
		let proposal_len = proposal.encoded_size();

		let proposal_hash = T::Hashing::hash_of(&proposal);
		ensure!(
			!<ProposalOf<T, I>>::contains_key(dao_id, proposal_hash),
			Error::<T, I>::DuplicateProposal
		);

		let seats = Self::members(dao_id).len() as u32;

		let policy = T::DaoProvider::policy(dao_id)?;
		let threshold = match policy.approve_origin {
			DaoPolicyProportion::AtLeast((count, total)) =>
				Permill::from_rational(count, total).mul_floor(seats),
			DaoPolicyProportion::MoreThan((count, total)) =>
				Permill::from_rational(count, total).mul_ceil(seats),
		};

		let active_proposals =
			<Proposals<T, I>>::try_mutate(dao_id, |proposals| -> Result<usize, DispatchError> {
				proposals.try_push(proposal_hash).map_err(|_| Error::<T, I>::TooManyProposals)?;
				Ok(proposals.len())
			})?;

		// Make sure using Inline type of Bounded
		let (proposal_dispatch_data, _) = T::Preimages::peek(&proposal)?;

		let index = Self::proposal_count(dao_id);
		<ProposalCount<T, I>>::mutate(dao_id, |i| *i += 1);
		<ProposalOf<T, I>>::insert(dao_id, proposal_hash, proposal);
		let now = frame_system::Pallet::<T>::block_number();
		let end = now.saturating_add(policy.proposal_period.into());
		let votes = {
			Votes {
				index,
				threshold,
				ayes: BoundedVec::<T::AccountId, T::MaxVotes>::default(),
				nays: BoundedVec::<T::AccountId, T::MaxVotes>::default(),
				end,
			}
		};
		<Voting<T, I>>::insert(dao_id, proposal_hash, votes);

		let account_id = T::DaoProvider::dao_account_id(dao_id);
		let origin = DaoRawOrigin::Dao(account_id).into();

		let proposal_weight_bound = {
			let b = length_bound;
			let m = T::MaxMembers::get();
			let p1: Weight = Default::default();
			let p2 = T::MaxProposals::get();

			T::WeightInfo::close_early_approved(b, m, p2)
				.max(T::WeightInfo::close_early_disapproved(m, p2))
				.max(T::WeightInfo::close_approved(b, m, p2))
				.max(T::WeightInfo::close_disapproved(m, p2))
				.saturating_add(p1.into())
		};

		let call = CallOf::<T, I>::from(Call::close_scheduled_proposal {
			dao_id,
			proposal_hash,
			index,
			proposal_weight_bound,
			length_bound: Default::default(),
		});

		// Scheduling proposal close
		T::Scheduler::schedule_named(
			(DAO_COLLECTIVE_ID, dao_id, proposal_hash, index).encode_into(),
			DispatchTime::At(end),
			None,
			50,
			origin,
			T::Preimages::bound(call)?.transmute(),
		)?;

		Self::deposit_event(Event::Proposed {
			dao_id,
			account: who,
			proposal_index: index,
			proposal_hash,
			proposal: proposal_dispatch_data,
			threshold,
			meta,
		});
		Ok((proposal_len as u32, active_proposals as u32))
	}

	/// Add an aye or nay vote for the member to the given proposal, returns true if it's the first
	/// vote of the member in the motion
	pub fn do_vote(
		who: T::AccountId,
		dao_id: DaoId,
		proposal: T::Hash,
		index: ProposalIndex,
		approve: bool,
	) -> Result<bool, DispatchError> {
		let mut voting = Self::voting(dao_id, proposal).ok_or(Error::<T, I>::ProposalMissing)?;
		ensure!(voting.index == index, Error::<T, I>::WrongIndex);

		// Proposal voting is not allowed if the voting period has ended.
		ensure!(frame_system::Pallet::<T>::block_number() <= voting.end, Error::<T, I>::Expired);

		let position_yes = voting.ayes.iter().position(|a| a == &who);
		let position_no = voting.nays.iter().position(|a| a == &who);

		// Detects first vote of the member in the motion
		let is_account_voting_first_time = position_yes.is_none() && position_no.is_none();

		if approve {
			if position_yes.is_none() {
				let mut ayes = voting.ayes.to_vec();
				ayes.push(who.clone());

				voting.ayes = BoundedVec::<T::AccountId, T::MaxVotes>::try_from(ayes)
					.map_err(|_| Error::<T, I>::TooManyVotes)?;
			} else {
				return Err(Error::<T, I>::DuplicateVote.into())
			}
			if let Some(pos) = position_no {
				voting.nays.swap_remove(pos);
			}
		} else {
			if position_no.is_none() {
				let mut nays = voting.nays.to_vec();
				nays.push(who.clone());

				voting.nays = BoundedVec::<T::AccountId, T::MaxVotes>::try_from(nays)
					.map_err(|_| Error::<T, I>::TooManyVotes)?;
			} else {
				return Err(Error::<T, I>::DuplicateVote.into())
			}
			if let Some(pos) = position_yes {
				voting.ayes.swap_remove(pos);
			}
		}

		let yes_votes = voting.ayes.len() as MemberCount;
		let no_votes = voting.nays.len() as MemberCount;
		Self::deposit_event(Event::Voted {
			dao_id,
			account: who,
			proposal_index: index,
			proposal_hash: proposal,
			voted: approve,
			yes: yes_votes,
			no: no_votes,
		});

		Voting::<T, I>::insert(dao_id, proposal, voting);

		Ok(is_account_voting_first_time)
	}

	/// Close a vote that is either approved, disapproved or whose voting period has ended.
	pub fn do_close(
		dao_id: DaoId,
		proposal_hash: T::Hash,
		index: ProposalIndex,
		proposal_weight_bound: Weight,
		length_bound: u32,
	) -> DispatchResultWithPostInfo {
		let voting = Self::voting(dao_id, proposal_hash).ok_or(Error::<T, I>::ProposalMissing)?;
		ensure!(voting.index == index, Error::<T, I>::WrongIndex);

		let mut no_votes = voting.nays.len() as MemberCount;
		let mut yes_votes = voting.ayes.len() as MemberCount;
		let seats = Self::members(dao_id).len() as MemberCount;
		let approved = yes_votes > 0 && yes_votes >= voting.threshold;
		let disapproved = seats.saturating_sub(no_votes) <= voting.threshold;
		// Allow (dis-)approving the proposal as soon as there are enough votes.
		if approved {
			let (proposal, len) = Self::validate_and_get_proposal(
				dao_id,
				&proposal_hash,
				length_bound,
				proposal_weight_bound,
			)?;
			Self::deposit_event(Event::Closed {
				dao_id,
				proposal_index: index,
				proposal_hash,
				yes: yes_votes,
				no: no_votes,
			});
			let (proposal_weight, proposal_count) =
				Self::do_approve_proposal(dao_id, seats, yes_votes, index, proposal_hash, proposal);
			return Ok((
				Some(
					T::WeightInfo::close_early_approved(len as u32, seats, proposal_count)
						.saturating_add(proposal_weight),
				),
				Pays::Yes,
			)
				.into())
		} else if disapproved {
			Self::deposit_event(Event::Closed {
				dao_id,
				proposal_index: index,
				proposal_hash,
				yes: yes_votes,
				no: no_votes,
			});
			let proposal_count = Self::do_disapprove_proposal(dao_id, index, proposal_hash);
			return Ok((
				Some(T::WeightInfo::close_early_disapproved(seats, proposal_count)),
				Pays::No,
			)
				.into())
		}

		// Only allow actual closing of the proposal after the voting period has ended.
		ensure!(frame_system::Pallet::<T>::block_number() >= voting.end, Error::<T, I>::TooEarly);

		// default voting strategy.
		let default = T::DefaultVote::default_vote(yes_votes, no_votes, seats);

		let abstentions = seats - (yes_votes + no_votes);
		match default {
			true => yes_votes += abstentions,
			false => no_votes += abstentions,
		}
		let approved = yes_votes >= voting.threshold;

		if approved {
			let (proposal, len) = Self::validate_and_get_proposal(
				dao_id,
				&proposal_hash,
				length_bound,
				proposal_weight_bound,
			)?;
			Self::deposit_event(Event::Closed {
				dao_id,
				proposal_index: index,
				proposal_hash,
				yes: yes_votes,
				no: no_votes,
			});
			let (proposal_weight, proposal_count) =
				Self::do_approve_proposal(dao_id, seats, yes_votes, index, proposal_hash, proposal);
			Ok((
				Some(
					T::WeightInfo::close_approved(len as u32, seats, proposal_count)
						.saturating_add(proposal_weight),
				),
				Pays::Yes,
			)
				.into())
		} else {
			Self::deposit_event(Event::Closed {
				dao_id,
				proposal_hash,
				proposal_index: index,
				yes: yes_votes,
				no: no_votes,
			});
			let proposal_count = Self::do_disapprove_proposal(dao_id, index, proposal_hash);
			Ok((Some(T::WeightInfo::close_disapproved(seats, proposal_count)), Pays::No).into())
		}
	}

	/// Ensure that the right proposal bounds were passed and get the proposal from storage.
	///
	/// Checks the length in storage via `storage::read` which adds an extra `size_of::<u32>() == 4`
	/// to the length.
	fn validate_and_get_proposal(
		dao_id: DaoId,
		hash: &T::Hash,
		_length_bound: u32,
		_weight_bound: Weight,
	) -> Result<(<T as Config<I>>::Proposal, usize), DispatchError> {
		let key = ProposalOf::<T, I>::hashed_key_for(dao_id, hash);
		// read the length of the proposal storage entry directly
		let proposal_len =
			storage::read(&key, &mut [0; 0], 0).ok_or(Error::<T, I>::ProposalMissing)?;
		let proposal =
			ProposalOf::<T, I>::get(dao_id, hash).ok_or(Error::<T, I>::ProposalMissing)?;

		let (call, _lookup_len) = match T::Preimages::peek(&proposal) {
			Ok(c) => c,
			Err(_) => return Err(Error::<T, I>::ProposalMissing.into()),
		};

		Ok((call, proposal_len as usize))
	}

	fn do_approve_proposal(
		dao_id: DaoId,
		seats: MemberCount,
		yes_votes: MemberCount,
		proposal_index: ProposalIndex,
		proposal_hash: T::Hash,
		proposal: <T as Config<I>>::Proposal,
	) -> (Weight, u32) {
		Self::deposit_event(Event::Approved { dao_id, proposal_index, proposal_hash });

		let dispatch_weight = proposal.get_dispatch_info().weight;
		let origin = RawOrigin::Members(yes_votes, seats).into();
		let result = proposal.dispatch(origin);
		Self::deposit_event(Event::Executed {
			dao_id,
			proposal_index,
			proposal_hash,
			result: result.map(|_| ()).map_err(|e| e.error),
		});
		// default to the dispatch info weight for safety
		let proposal_weight = get_result_weight(result).unwrap_or(dispatch_weight); // P1

		let proposal_count = Self::remove_proposal(dao_id, proposal_hash);

		(proposal_weight, proposal_count)
	}

	/// Removes a proposal from the pallet, and deposit the `Disapproved` event.
	pub fn do_disapprove_proposal(
		dao_id: DaoId,
		proposal_index: ProposalIndex,
		proposal_hash: T::Hash,
	) -> u32 {
		// disapproved
		Self::deposit_event(Event::Disapproved { dao_id, proposal_index, proposal_hash });
		Self::remove_proposal(dao_id, proposal_hash)
	}

	// Removes a proposal from the pallet, cleaning up votes and the vector of proposals.
	fn remove_proposal(dao_id: DaoId, proposal_hash: T::Hash) -> u32 {
		// remove proposal and vote
		ProposalOf::<T, I>::remove(dao_id, proposal_hash);
		Voting::<T, I>::remove(dao_id, proposal_hash);
		let num_proposals = Proposals::<T, I>::mutate(dao_id, |proposals| {
			proposals.retain(|h| h != &proposal_hash);
			proposals.len() + 1 // calculate weight based on original length
		});
		num_proposals as u32
	}

	fn close_proposal_task_id(
		dao_id: DaoId,
		proposal_hash: T::Hash,
		index: ProposalIndex,
	) -> TaskName {
		(DAO_COLLECTIVE_ID, dao_id, proposal_hash, index).encode_into()
	}
}

impl<T: Config<I>, I: 'static> ChangeDaoMembers<DaoId, T::AccountId> for Pallet<T, I> {
	/// Update the members of the collective. Votes are updated.
	///
	/// NOTE: Does not enforce the expected `MaxMembers` limit on the amount of members, but
	///       the weight estimations rely on it to estimate dispatchable weight.
	fn change_members_sorted(
		dao_id: DaoId,
		_incoming: &[T::AccountId],
		outgoing: &[T::AccountId],
		new: &[T::AccountId],
	) {
		if new.len() > T::MaxMembers::get() as usize {
			log::error!(
				target: "runtime::collective",
				"New members count ({}) exceeds maximum amount of members expected ({}).",
				new.len(),
				T::MaxMembers::get(),
			);
		}
		// remove accounts from all current voting in motions.
		let mut outgoing = outgoing.to_vec();
		outgoing.sort();
		for h in Self::proposals(dao_id).into_iter() {
			<Voting<T, I>>::mutate(dao_id, h, |v| {
				if let Some(mut votes) = v.take() {
					let ayes: Vec<T::AccountId> = votes
						.ayes
						.iter()
						.cloned()
						.filter(|i| outgoing.binary_search(i).is_err())
						.collect();
					votes.ayes = BoundedVec::<T::AccountId, T::MaxVotes>::try_from(ayes).unwrap();

					let nays: Vec<T::AccountId> = votes
						.nays
						.iter()
						.cloned()
						.filter(|i| outgoing.binary_search(i).is_err())
						.collect();
					votes.nays = BoundedVec::<T::AccountId, T::MaxVotes>::try_from(nays).unwrap();
					*v = Some(votes);
				}
			});
		}
		let members = BoundedVec::<T::AccountId, T::MaxMembers>::try_from(new.to_vec()).unwrap();
		Members::<T, I>::insert(dao_id, members);
	}
}

/// Ensure that the origin `o` represents at least `n` members. Returns `Ok` or an `Err`
/// otherwise.
pub fn ensure_members<OuterOrigin, AccountId, I>(
	o: OuterOrigin,
	n: MemberCount,
) -> result::Result<MemberCount, &'static str>
where
	OuterOrigin: Into<result::Result<RawOrigin<AccountId, I>, OuterOrigin>>,
{
	match o.into() {
		Ok(RawOrigin::Members(x, _)) if x >= n => Ok(n),
		_ => Err("bad origin: expected to be a threshold number of members"),
	}
}

pub struct EnsureMember<AccountId, I: 'static>(PhantomData<(AccountId, I)>);
impl<
		O: Into<Result<RawOrigin<AccountId, I>, O>> + From<RawOrigin<AccountId, I>>,
		I,
		AccountId: Decode,
	> EnsureOrigin<O> for EnsureMember<AccountId, I>
{
	type Success = AccountId;
	fn try_origin(o: O) -> Result<Self::Success, O> {
		o.into().and_then(|o| match o {
			RawOrigin::Member(id) => Ok(id),
			r => Err(O::from(r)),
		})
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin() -> Result<O, ()> {
		let zero_account_id =
			AccountId::decode(&mut sp_runtime::traits::TrailingZeroInput::zeroes())
				.expect("infinite length input; no invalid inputs for type; qed");
		Ok(O::from(RawOrigin::Member(zero_account_id)))
	}
}

pub struct EnsureMembers<AccountId, I: 'static, const N: u32>(PhantomData<(AccountId, I)>);
impl<
		O: Into<Result<RawOrigin<AccountId, I>, O>> + From<RawOrigin<AccountId, I>>,
		AccountId,
		I,
		const N: u32,
	> EnsureOrigin<O> for EnsureMembers<AccountId, I, N>
{
	type Success = (MemberCount, MemberCount);
	fn try_origin(o: O) -> Result<Self::Success, O> {
		o.into().and_then(|o| match o {
			RawOrigin::Members(n, m) if n >= N => Ok((n, m)),
			r => Err(O::from(r)),
		})
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin() -> Result<O, ()> {
		Ok(O::from(RawOrigin::Members(N, N)))
	}
}

pub struct EnsureDaoOriginWithArg<AccountId, I: 'static = ()>(PhantomData<(AccountId, I)>);
impl<O: Into<Result<RawOrigin<AccountId, I>, O>> + From<RawOrigin<AccountId, I>>, AccountId, I>
	EnsureOriginWithArg<O, DaoOrigin<AccountId>> for EnsureDaoOriginWithArg<AccountId, I>
{
	type Success = ();

	fn try_origin(o: O, arg: &DaoOrigin<AccountId>) -> Result<Self::Success, O> {
		o.into().and_then(|o| match o {
			RawOrigin::Members(count, total) => {
				if match arg.proportion {
					DaoPolicyProportion::AtLeast((n, d)) => count * d >= n * total,
					DaoPolicyProportion::MoreThan((n, d)) => count * d > n * total,
				} {
					return Ok(())
				}

				Err(O::from(o))
			},
			r => Err(O::from(r)),
		})
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin(dao_origin: &DaoOrigin<AccountId>) -> Result<O, ()> {
		let proportion = &dao_origin.proportion;
		match proportion {
			DaoPolicyProportion::AtLeast(at_least) =>
				Ok(O::from(RawOrigin::Members(at_least.0, at_least.1))),
			DaoPolicyProportion::MoreThan(more_than) =>
				Ok(O::from(RawOrigin::Members(more_than.0, more_than.1))),
		}
	}
}

pub struct EitherOfDiverseWithArg<L, R>(PhantomData<(L, R)>);
impl<
		OuterOrigin,
		L: EnsureOriginWithArg<OuterOrigin, Argument>,
		R: EnsureOriginWithArg<OuterOrigin, Argument>,
		Argument,
	> EnsureOriginWithArg<OuterOrigin, Argument> for EitherOfDiverseWithArg<L, R>
{
	type Success = Either<L::Success, R::Success>;
	fn try_origin(o: OuterOrigin, arg: &Argument) -> Result<Self::Success, OuterOrigin> {
		L::try_origin(o, arg)
			.map_or_else(|o| R::try_origin(o, arg).map(Either::Right), |o| Ok(Either::Left(o)))
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin(dao_origin: &Argument) -> Result<OuterOrigin, ()> {
		L::try_successful_origin(dao_origin).or_else(|()| R::try_successful_origin(dao_origin))
	}
}

impl<T: Config<I>, I: 'static> InitializeDaoMembers<DaoId, T::AccountId> for Pallet<T, I> {
	fn initialize_members(dao_id: DaoId, members: Vec<T::AccountId>) -> Result<(), DispatchError> {
		// considering we've checked everything in membership pallet
		<Members<T, I>>::insert(
			dao_id,
			BoundedVec::<T::AccountId, T::MaxMembers>::try_from(members)
				.map_err(|_| Error::<T, I>::TooManyMembers)?,
		);

		Ok(())
	}
}
