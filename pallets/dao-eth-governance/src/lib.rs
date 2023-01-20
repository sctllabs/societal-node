#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "128"]

use codec::Output;
use scale_info::{prelude::*, TypeInfo};
use sp_io::storage;
use sp_runtime::{traits::Hash, Either, RuntimeDebug};
use sp_std::{marker::PhantomData, prelude::*, result, str};

use dao_primitives::{
	ApprovePropose, ApproveVote, ChangeDaoMembers, DaoOrigin, DaoPolicy, DaoPolicyProportion,
	DaoProvider, InitializeDaoMembers, PendingProposal, PendingVote, RawOrigin,
};

use frame_support::{
	codec::{Decode, Encode, MaxEncodedLen},
	dispatch::{
		DispatchError, DispatchInfo, DispatchResultWithPostInfo, Dispatchable, GetDispatchInfo,
		Pays, PostDispatchInfo,
	},
	ensure,
	traits::{
		Backing, Bounded, EnsureOrigin, EnsureOriginWithArg, Get, GetBacking, QueryPreimage,
		StorageVersion, StorePreimage,
	},
	weights::Weight,
	Parameter,
};
use sp_core::bounded::BoundedVec;

mod vote;

pub use pallet::*;

/// Dao ID. Just a `u32`.
pub type DaoId = u32;

/// Token Supply
pub type TokenSupply = u128;

/// Simple index type for proposal counting.
pub type ProposalIndex = u32;

pub type BoundedProposal<T> = Bounded<<T as Config>::Proposal>;

/// Info for keeping track of a motion being voted on.
#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct Votes<BlockNumber, VotingSet> {
	/// The proposal's unique index.
	index: ProposalIndex,
	/// The number of approval votes that are needed to pass the motion.
	threshold: TokenSupply,
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
	use dao_primitives::{AccountTokenBalance, PendingProposal, PendingVote};
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(4);

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The outer event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The outer origin type.
		type RuntimeOrigin: From<RawOrigin<Self::AccountId>>;

		/// The outer call dispatch type.
		type Proposal: Parameter
			+ Dispatchable<
				RuntimeOrigin = <Self as Config>::RuntimeOrigin,
				PostInfo = PostDispatchInfo,
			> + From<frame_system::Call<Self>>
			+ GetDispatchInfo;

		/// Maximum number of proposals allowed to be active in parallel.
		type MaxProposals: Get<ProposalIndex>;

		/// The maximum number of votes(ayes/nays) for a proposal.
		#[pallet::constant]
		type MaxVotes: Get<u32>;

		type DaoProvider: DaoProvider<
			<Self as frame_system::Config>::Hash,
			Id = u32,
			AccountId = Self::AccountId,
			Policy = DaoPolicy,
		>;

		/// The preimage provider with which we look up call hashes to get the call.
		type Preimages: QueryPreimage + StorePreimage;
	}

	/// The hashes of the active proposals by Dao.
	#[pallet::storage]
	#[pallet::getter(fn proposals)]
	pub type Proposals<T: Config> =
		StorageMap<_, Twox64Concat, DaoId, BoundedVec<T::Hash, T::MaxProposals>, ValueQuery>;

	/// Actual proposal for a given hash, if it's current.
	#[pallet::storage]
	#[pallet::getter(fn proposal_of)]
	pub type ProposalOf<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		DaoId,
		Identity,
		T::Hash,
		BoundedProposal<T>,
		OptionQuery,
	>;

	/// Actual pending proposal for a given hash.
	#[pallet::storage]
	#[pallet::getter(fn pending_proposal_of)]
	pub type PendingProposalOf<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		DaoId,
		Identity,
		T::Hash,
		(PendingProposal<T::AccountId>, BoundedProposal<T>),
		OptionQuery,
	>;

	/// Votes on a given proposal, if it is ongoing.
	#[pallet::storage]
	#[pallet::getter(fn voting)]
	pub type Voting<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		DaoId,
		Identity,
		T::Hash,
		Votes<T::BlockNumber, BoundedVec<T::AccountId, T::MaxVotes>>,
		OptionQuery,
	>;

	/// Actual pending proposal for a given hash.
	#[pallet::storage]
	#[pallet::getter(fn pending_voting)]
	pub type PendingVoting<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		DaoId,
		Identity,
		T::Hash,
		PendingVote<T::AccountId, T::Hash>,
		OptionQuery,
	>;

	/// Proposals so far.
	#[pallet::storage]
	#[pallet::getter(fn proposal_count)]
	pub type ProposalCount<T: Config> = StorageMap<_, Twox64Concat, DaoId, u32, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		// Proposal pending approval
		ProposalPendingApproval {
			dao_id: DaoId,
			account: T::AccountId,
			proposal_hash: T::Hash,
		},
		/// A motion (given hash) has been proposed (by given account) with a threshold
		Proposed {
			dao_id: DaoId,
			account: T::AccountId,
			proposal_index: ProposalIndex,
			proposal_hash: T::Hash,
			threshold: TokenSupply,
		},
		/// Proposal vote pending approval
		VotePendingApproval {
			dao_id: DaoId,
			account: T::AccountId,
			proposal_hash: T::Hash,
			proposal_index: ProposalIndex,
			voted: bool,
		},
		/// A motion (given hash) has been voted on by given account, leaving
		/// a tally (yes votes and no votes given respectively as `TokenSupply`).
		Voted {
			dao_id: DaoId,
			account: T::AccountId,
			proposal_hash: T::Hash,
			voted: bool,
			yes: TokenSupply,
			no: TokenSupply,
		},
		/// A motion was approved by the required threshold.
		Approved {
			dao_id: DaoId,
			proposal_hash: T::Hash,
		},
		/// A motion was not approved by the required threshold.
		Disapproved {
			dao_id: DaoId,
			proposal_hash: T::Hash,
		},
		/// A motion was executed; result will be `Ok` if it returned without error.
		Executed {
			dao_id: DaoId,
			proposal_hash: T::Hash,
			result: DispatchResult,
		},
		/// A single member did some action; result will be `Ok` if it returned without error.
		MemberExecuted {
			dao_id: DaoId,
			proposal_hash: T::Hash,
			result: DispatchResult,
		},
		/// A proposal was closed because its threshold was reached or after its duration was up.
		Closed {
			dao_id: DaoId,
			proposal_hash: T::Hash,
			yes: TokenSupply,
			no: TokenSupply,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
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
		/// There can only be a maximum of `MaxVotes` votes for proposal.
		TooManyVotes,
		/// There can only be a maximum of `MaxMembers` votes for proposal.
		TooManyMembers,
		/// Action is not allowed for non-eth DAOs
		NotEthDao,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add a new proposal to either be voted on or executed directly.
		///
		/// Requires the sender to be member.
		#[pallet::weight(10_1000)]
		pub fn propose(
			origin: OriginFor<T>,
			dao_id: DaoId,
			proposal: Box<<T as Config>::Proposal>,
			#[pallet::compact] length_bound: u32,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let proposal_hash = T::Hashing::hash_of(&proposal);

			let account_token_balance = T::DaoProvider::ensure_proposal_allowed(
				dao_id,
				&who,
				0, //TODO
				proposal_hash,
				length_bound,
				true,
			)?;

			match account_token_balance {
				AccountTokenBalance::Offchain { .. } => {
					let pending_proposal =
						PendingProposal { who: who.clone(), threshold: 0_u32, length_bound }; //TODO

					let proposal = T::Preimages::bound(proposal)?.transmute();

					<PendingProposalOf<T>>::insert(
						dao_id,
						proposal_hash,
						(pending_proposal, proposal),
					);

					Self::deposit_event(Event::ProposalPendingApproval {
						dao_id,
						account: who,
						proposal_hash,
					});

					Ok(Default::default())
				},
				_ => Err(Error::<T>::NotEthDao.into()),
			}
		}

		/// Add an aye or nay vote for the sender to the given proposal.
		///
		/// Requires the sender to be a member.
		///
		/// Transaction fees will be waived if the member is voting on any particular proposal
		/// for the first time and the call is successful. Subsequent vote changes will charge a
		/// fee.
		#[pallet::weight(10_000)]
		pub fn vote(
			origin: OriginFor<T>,
			dao_id: DaoId,
			proposal: T::Hash,
			#[pallet::compact] index: ProposalIndex,
			approve: bool,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			// let members = Self::members(dao_id);
			// ensure!(members.contains(&who), Error::<T>::NotMember);

			let pending_vote = PendingVote {
				who: who.clone(),
				proposal_hash: proposal,
				proposal_index: index,
				vote: approve,
			};

			let pending_vote_hash = T::Hashing::hash_of(&pending_vote);

			let account_token_balance =
				T::DaoProvider::ensure_voting_allowed(dao_id, &who, pending_vote_hash, false)?;

			match account_token_balance {
				AccountTokenBalance::Offchain { .. } => {
					// TODO: add checks for vec size limits

					<PendingVoting<T>>::insert(dao_id, pending_vote_hash, pending_vote);

					Self::deposit_event(Event::VotePendingApproval {
						dao_id,
						account: who,
						proposal_hash: proposal,
						proposal_index: index,
						voted: approve,
					});

					return Ok(Default::default())
				},
				_ => Err(Error::<T>::NotEthDao.into()),
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
		/// + `length_bound`: The upper bound for the length of the proposal in storage. Checked via
		/// `storage::read` so it is `size_of::<u32>() == 4` larger than the pure length.
		#[pallet::weight(10_000)]
		pub fn close(
			origin: OriginFor<T>,
			dao_id: DaoId,
			proposal_hash: T::Hash,
			#[pallet::compact] index: ProposalIndex,
			proposal_weight_bound: Weight,
			#[pallet::compact] length_bound: u32,
		) -> DispatchResultWithPostInfo {
			let _ = ensure_signed(origin)?;

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

impl<T: Config> Pallet<T> {
	/// Add a new proposal to be voted.
	pub fn do_propose_proposed(
		who: T::AccountId,
		dao_id: DaoId,
		threshold: TokenSupply,
		proposal: BoundedProposal<T>,
		length_bound: u32,
	) -> Result<(u32, u32), DispatchError> {
		let proposal_len = proposal.encoded_size();
		ensure!(proposal_len <= length_bound as usize, Error::<T>::WrongProposalLength);

		let proposal_hash = T::Hashing::hash_of(&proposal);
		ensure!(
			!<ProposalOf<T>>::contains_key(dao_id, proposal_hash),
			Error::<T>::DuplicateProposal
		);

		let policy = T::DaoProvider::policy(dao_id)?;
		// let threshold = match policy.approve_origin {
		// 	DaoPolicyProportion::AtLeast((threshold, _)) => threshold,
		// 	DaoPolicyProportion::MoreThan((threshold, _)) => threshold,
		// };

		let active_proposals =
			<Proposals<T>>::try_mutate(dao_id, |proposals| -> Result<usize, DispatchError> {
				proposals.try_push(proposal_hash).map_err(|_| Error::<T>::TooManyProposals)?;
				Ok(proposals.len())
			})?;

		let index = Self::proposal_count(dao_id);
		<ProposalCount<T>>::mutate(dao_id, |i| *i += 1);
		<ProposalOf<T>>::insert(dao_id, proposal_hash, proposal);
		let votes = {
			let end = frame_system::Pallet::<T>::block_number() + policy.proposal_period.into();
			Votes {
				index,
				threshold,
				ayes: BoundedVec::<T::AccountId, T::MaxVotes>::default(),
				nays: BoundedVec::<T::AccountId, T::MaxVotes>::default(),
				end,
			}
		};
		<Voting<T>>::insert(dao_id, proposal_hash, votes);

		Self::deposit_event(Event::Proposed {
			dao_id,
			account: who,
			proposal_index: index,
			proposal_hash,
			threshold,
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
		let mut voting = Self::voting(dao_id, &proposal).ok_or(Error::<T>::ProposalMissing)?;
		ensure!(voting.index == index, Error::<T>::WrongIndex);

		let position_yes = voting.ayes.iter().position(|a| a == &who);
		let position_no = voting.nays.iter().position(|a| a == &who);

		// Detects first vote of the member in the motion
		let is_account_voting_first_time = position_yes.is_none() && position_no.is_none();

		if approve {
			if position_yes.is_none() {
				let mut ayes = voting.ayes.to_vec();
				ayes.push(who.clone());

				voting.ayes = BoundedVec::<T::AccountId, T::MaxVotes>::try_from(ayes)
					.map_err(|_| Error::<T>::TooManyVotes)?;
			} else {
				return Err(Error::<T>::DuplicateVote.into())
			}
			if let Some(pos) = position_no {
				voting.nays.swap_remove(pos);
			}
		} else {
			if position_no.is_none() {
				let mut nays = voting.nays.to_vec();
				nays.push(who.clone());

				voting.nays = BoundedVec::<T::AccountId, T::MaxVotes>::try_from(nays)
					.map_err(|_| Error::<T>::TooManyVotes)?;
			} else {
				return Err(Error::<T>::DuplicateVote.into())
			}
			if let Some(pos) = position_yes {
				voting.ayes.swap_remove(pos);
			}
		}

		let yes_votes = voting.ayes.len() as TokenSupply;
		let no_votes = voting.nays.len() as TokenSupply;
		Self::deposit_event(Event::Voted {
			dao_id,
			account: who,
			proposal_hash: proposal,
			voted: approve,
			yes: yes_votes,
			no: no_votes,
		});

		Voting::<T>::insert(dao_id, &proposal, voting);

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
		let voting = Self::voting(dao_id, &proposal_hash).ok_or(Error::<T>::ProposalMissing)?;
		ensure!(voting.index == index, Error::<T>::WrongIndex);

		let mut no_votes = voting.nays.len() as TokenSupply;
		let mut yes_votes = voting.ayes.len() as TokenSupply;
		// let seats = Self::members(dao_id).len() as TokenSupply;
		let seats = 1_u128;
		let approved = yes_votes >= voting.threshold;
		let disapproved = seats.saturating_sub(no_votes) < voting.threshold;
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
				proposal_hash,
				yes: yes_votes,
				no: no_votes,
			});
			let (proposal_weight, proposal_count) =
				Self::do_approve_proposal(dao_id, seats, yes_votes, proposal_hash, proposal);
			return Ok((
				Some(
					// T::WeightInfo::close_early_approved(len as u32, seats, proposal_count)
					// 	.saturating_add(proposal_weight),
					Weight::from_ref_time(0),
				),
				Pays::Yes,
			)
				.into())
		} else if disapproved {
			Self::deposit_event(Event::Closed {
				dao_id,
				proposal_hash,
				yes: yes_votes,
				no: no_votes,
			});
			let proposal_count = Self::do_disapprove_proposal(dao_id, proposal_hash);
			return Ok((
				// Some(T::WeightInfo::close_early_disapproved(seats, proposal_count)),
				Some(Weight::from_ref_time(0)),
				Pays::No,
			)
				.into())
		}

		// Only allow actual closing of the proposal after the voting period has ended.
		ensure!(frame_system::Pallet::<T>::block_number() >= voting.end, Error::<T>::TooEarly);

		// default voting strategy.
		// let default = T::DefaultVote::default_vote(yes_votes, no_votes, seats);

		let abstentions = seats - (yes_votes + no_votes);
		// match default {
		// 	true => yes_votes += abstentions,
		// 	false => no_votes += abstentions,
		// }
		// TODO
		no_votes += abstentions;

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
				proposal_hash,
				yes: yes_votes,
				no: no_votes,
			});
			let (proposal_weight, proposal_count) =
				Self::do_approve_proposal(dao_id, seats, yes_votes, proposal_hash, proposal);
			Ok((
				Some(
					// T::WeightInfo::close_approved(len as u32, seats, proposal_count)
					// 	.saturating_add(proposal_weight),
					Weight::from_ref_time(0),
				),
				Pays::Yes,
			)
				.into())
		} else {
			Self::deposit_event(Event::Closed {
				dao_id,
				proposal_hash,
				yes: yes_votes,
				no: no_votes,
			});
			let proposal_count = Self::do_disapprove_proposal(dao_id, proposal_hash);
			// Ok((Some(T::WeightInfo::close_disapproved(seats, proposal_count)), Pays::No).into())
			Ok((Some(Weight::from_ref_time(0)), Pays::No).into())
		}
	}

	/// Ensure that the right proposal bounds were passed and get the proposal from storage.
	///
	/// Checks the length in storage via `storage::read` which adds an extra `size_of::<u32>() == 4`
	/// to the length.
	fn validate_and_get_proposal(
		dao_id: DaoId,
		hash: &T::Hash,
		length_bound: u32,
		weight_bound: Weight,
	) -> Result<(<T as Config>::Proposal, usize), DispatchError> {
		let key = ProposalOf::<T>::hashed_key_for(dao_id, hash);
		// read the length of the proposal storage entry directly
		let proposal_len =
			storage::read(&key, &mut [0; 0], 0).ok_or(Error::<T>::ProposalMissing)?;
		ensure!(proposal_len <= length_bound, Error::<T>::WrongProposalLength);
		let proposal = ProposalOf::<T>::get(dao_id, hash).ok_or(Error::<T>::ProposalMissing)?;

		let (call, _lookup_len) = match T::Preimages::peek(&proposal) {
			Ok(c) => c,
			Err(_) => return Err(Error::<T>::ProposalMissing.into()),
		};

		let proposal_weight = call.get_dispatch_info().weight;

		ensure!(proposal_weight.all_lte(weight_bound), Error::<T>::WrongProposalWeight);
		Ok((call, proposal_len as usize))
	}

	/// Weight:
	/// If `approved`:
	/// - the weight of `proposal` preimage.
	/// - two events deposited.
	/// - two removals, one mutation.
	/// - computation and i/o `O(P + L)` where:
	///   - `P` is number of active proposals,
	///   - `L` is the encoded length of `proposal` preimage.
	///
	/// If not `approved`:
	/// - one event deposited.
	/// Two removals, one mutation.
	/// Computation and i/o `O(P)` where:
	/// - `P` is number of active proposals
	fn do_approve_proposal(
		dao_id: DaoId,
		seats: TokenSupply,
		yes_votes: TokenSupply,
		proposal_hash: T::Hash,
		proposal: <T as Config>::Proposal,
	) -> (Weight, u32) {
		Self::deposit_event(Event::Approved { dao_id, proposal_hash });

		let dao_account_id = T::DaoProvider::dao_account_id(dao_id);

		let dispatch_weight = proposal.get_dispatch_info().weight;
		let origin = RawOrigin::Dao(dao_account_id).into();
		let result = proposal.dispatch(origin);
		Self::deposit_event(Event::Executed {
			dao_id,
			proposal_hash,
			result: result.map(|_| ()).map_err(|e| e.error),
		});
		// default to the dispatch info weight for safety
		let proposal_weight = get_result_weight(result).unwrap_or(dispatch_weight); // P1

		let proposal_count = Self::remove_proposal(dao_id, proposal_hash);
		(proposal_weight, proposal_count)
	}

	/// Removes a proposal from the pallet, and deposit the `Disapproved` event.
	pub fn do_disapprove_proposal(dao_id: DaoId, proposal_hash: T::Hash) -> u32 {
		// disapproved
		Self::deposit_event(Event::Disapproved { dao_id, proposal_hash });
		Self::remove_proposal(dao_id, proposal_hash)
	}

	// Removes a proposal from the pallet, cleaning up votes and the vector of proposals.
	fn remove_proposal(dao_id: DaoId, proposal_hash: T::Hash) -> u32 {
		// remove proposal and vote
		ProposalOf::<T>::remove(dao_id, &proposal_hash);
		Voting::<T>::remove(dao_id, &proposal_hash);
		let num_proposals = Proposals::<T>::mutate(dao_id, |proposals| {
			proposals.retain(|h| h != &proposal_hash);
			proposals.len() + 1 // calculate weight based on original length
		});
		num_proposals as u32
	}
}

impl<T: Config> ApprovePropose<DaoId, T::AccountId, T::Hash> for Pallet<T> {
	fn approve_propose(dao_id: DaoId, hash: T::Hash, approve: bool) -> Result<(), DispatchError> {
		let (pending_proposal, proposal) =
			<PendingProposalOf<T>>::take(dao_id, hash).expect("Pending Proposal not found");

		let PendingProposal { who, threshold, length_bound } = pending_proposal; //TODO

		// TODO: add threshold data from offchain
		if approve {
			// Self::do_propose_proposed(who, dao_id, threshold, proposal, length_bound)?;
			Self::do_propose_proposed(who, dao_id, 1, proposal, length_bound.into())?;
		}

		Ok(())
	}
}

impl<T: Config> ApproveVote<DaoId, T::AccountId, T::Hash> for Pallet<T> {
	fn approve_vote(dao_id: DaoId, hash: T::Hash, approve: bool) -> Result<(), DispatchError> {
		let PendingVote { who, proposal_hash, proposal_index, vote } =
			<PendingVoting<T>>::take(dao_id, hash).expect("Pending Vote not found");

		if approve {
			Self::do_vote(who, dao_id, proposal_hash, proposal_index, vote)?;
		}

		Ok(())
	}
}
