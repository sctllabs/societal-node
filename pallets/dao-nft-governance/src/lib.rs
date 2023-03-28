#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "128"]

use scale_info::{prelude::*, TypeInfo};
use sp_io::storage;
use sp_runtime::traits::Hash;
use sp_std::{prelude::*, str};

use dao_primitives::{DaoPolicy, DaoProvider, RawOrigin};

// TODO
use pallet_dao_democracy::vote_threshold::compare_rationals;

use frame_support::{
	codec::{Decode, Encode, MaxEncodedLen},
	dispatch::{
		DispatchError, DispatchResultWithPostInfo, Dispatchable, GetDispatchInfo, Pays,
		PostDispatchInfo,
	},
	ensure,
	traits::{tokens::Balance, Bounded, Get, QueryPreimage, StorageVersion, StorePreimage},
	weights::Weight,
	Parameter,
};
use sp_core::bounded::BoundedVec;
use sp_runtime::traits::{IntegerSquareRoot, Zero};
use sp_std::iter::Sum;

use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;

use crate::vote::{AccountVote, Vote};
pub use pallet::*;

pub mod vote;

type ItemOf<T> = <T as Config>::ItemId;

/// Dao ID. Just a `u32`.
pub type DaoId = u32;

/// Token Supply
pub type TokenSupply = u128;

/// Simple index type for proposal counting.
pub type ProposalIndex = u32;

pub type BlockNumber = u32;

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
	use crate::vote::Vote;
	use frame_support::traits::tokens::nonfungibles_v2::{Inspect, InspectEnumerable};

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

		/// The units in which we record balances.
		type Balance: Balance + Sum<Self::Balance> + Zero + From<u128>;

		/// The outer call dispatch type.
		type Proposal: Parameter
			+ Dispatchable<
				RuntimeOrigin = <Self as Config>::RuntimeOrigin,
				PostInfo = PostDispatchInfo,
			> + From<frame_system::Call<Self>>
			+ GetDispatchInfo;

		#[pallet::constant]
		type ProposalMetadataLimit: Get<u32>;

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
			NFTCollectionId = Self::NFTCollectionId,
		>;

		/// The preimage provider with which we look up call hashes to get the call.
		type Preimages: QueryPreimage + StorePreimage;

		type NFTCollectionId: Member + Parameter + MaxEncodedLen + Copy;

		type ItemId: Member + Parameter + MaxEncodedLen + Copy;

		type NFTProvider: Inspect<Self::AccountId, CollectionId = Self::NFTCollectionId, ItemId = Self::ItemId>
			+ InspectEnumerable<
				Self::AccountId,
				CollectionId = Self::NFTCollectionId,
				ItemId = Self::ItemId,
			>;
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

	/// Votes on a given proposal, if it is ongoing.
	#[pallet::storage]
	#[pallet::getter(fn voting)]
	pub type Voting<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		DaoId,
		Identity,
		T::Hash,
		Votes<T::BlockNumber, BoundedVec<AccountVote<T::AccountId, ItemOf<T>>, T::MaxVotes>>,
		OptionQuery,
	>;

	/// Proposals so far.
	#[pallet::storage]
	#[pallet::getter(fn proposal_count)]
	pub type ProposalCount<T: Config> = StorageMap<_, Twox64Concat, DaoId, u32, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A proposal (given hash) has been proposed (by given account) with a threshold
		Proposed {
			dao_id: DaoId,
			account: T::AccountId,
			proposal_index: ProposalIndex,
			proposal_hash: T::Hash,
			proposal: T::Proposal,
			threshold: TokenSupply,
			meta: BoundedVec<u8, T::ProposalMetadataLimit>,
		},
		/// A motion (given hash) has been voted on by given account, leaving
		/// a tally (yes votes and no votes given respectively as `TokenSupply`).
		Voted {
			dao_id: DaoId,
			account: T::AccountId,
			proposal_index: ProposalIndex,
			proposal_hash: T::Hash,
			vote: Vote<ItemOf<T>>,
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
			ayes: u128,
			nays: u128,
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
		/// Signer account is not equal to the account provided
		WrongAccountId,
		/// Invalid input
		InvalidInput,
		/// Metadata size exceeds the limits
		MetadataTooLong,
		/// Collection has too many items in it
		CollectionIsTooBig,
		/// Not supported 
		NotSupported,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add a new proposal to either be voted on or executed directly.
		///
		/// Requires the sender to be member.
		#[pallet::weight(10_1000)]
		#[pallet::call_index(0)]
		pub fn propose(
			origin: OriginFor<T>,
			dao_id: DaoId,
			proposal: Box<<T as Config>::Proposal>,
		) -> DispatchResultWithPostInfo {
			Self::propose_with_meta(origin, dao_id, proposal, None)
		}

		/// Adds a new proposal with temporary meta field for arbitrary data indexed by node indexer
		#[pallet::weight(10_1000)]
		#[pallet::call_index(1)]
		pub fn propose_with_meta(
			origin: OriginFor<T>,
			dao_id: DaoId,
			proposal: Box<<T as Config>::Proposal>,
			meta: Option<Vec<u8>>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let meta = match meta.clone() {
				Some(metadata) => BoundedVec::<u8, T::ProposalMetadataLimit>::try_from(metadata)
					.map_err(|_| Error::<T>::MetadataTooLong)?,
				None => Default::default(),
			};

			let dao_nft_collection_id = T::DaoProvider::dao_nft_collection_id(dao_id)?;

			match dao_nft_collection_id {
				None => return Err(Error::<T>::NotSupported.into()),
				Some(nft_collection_id) => {
					let mut owned_in_collection =
						T::NFTProvider::owned_in_collection(&nft_collection_id, &who);

					match owned_in_collection.next() {
						None => return Err(Error::<T>::NotMember.into()),
						Some(_) => {
							let proposal = T::Preimages::bound(proposal)?.transmute();

							let threshold = T::NFTProvider::items(&nft_collection_id).count();
							let threshold = u128::try_from(threshold)
								.map_err(|_| Error::<T>::CollectionIsTooBig)?;

							Self::do_propose_proposed(who, dao_id, threshold, proposal, meta)?;
						},
					}
				},
			}

			Ok(Default::default())
		}

		/// Add an aye or nay vote for the sender to the given proposal.
		#[pallet::weight(10_000)]
		#[pallet::call_index(2)]
		pub fn vote(
			origin: OriginFor<T>,
			dao_id: DaoId,
			proposal: T::Hash,
			#[pallet::compact] index: ProposalIndex,
			vote: Vote<ItemOf<T>>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let voting = Self::voting(dao_id, proposal).ok_or(Error::<T>::ProposalMissing)?;
			ensure!(voting.index == index, Error::<T>::WrongIndex);

			let Vote { aye: _, item } = vote;

			let dao_nft_collection_id = T::DaoProvider::dao_nft_collection_id(dao_id)?;

			match dao_nft_collection_id {
				None => return Err(Error::<T>::NotSupported.into()),
				Some(nft_collection_id) => {
					let owner = T::NFTProvider::owner(&nft_collection_id, &item);

					match owner {
						None => return Err(Error::<T>::NotMember.into()),
						Some(_) => {
							Self::do_vote(who, dao_id, proposal, index, vote)?;
						},
					}
				},
			}

			Ok(Default::default())
		}

		/// Close a vote that is either approved, disapproved or whose voting period has ended.
		///
		/// May be called by any signed account in order to finish voting and close the proposal.
		///
		/// If called before the end of the voting period it will only close the vote if it is
		/// has enough votes to be approved or disapproved.
		///
		/// If the close operation completes successfully with disapproval, the transaction fee will
		/// be waived. Otherwise execution of the approved operation will be charged to the caller.
		///
		/// + `proposal_weight_bound`: The maximum amount of weight consumed by executing the closed
		/// proposal.
		#[pallet::weight(10_000)]
		#[pallet::call_index(3)]
		pub fn close(
			origin: OriginFor<T>,
			dao_id: DaoId,
			proposal_hash: T::Hash,
			#[pallet::compact] index: ProposalIndex,
			proposal_weight_bound: Weight,
		) -> DispatchResultWithPostInfo {
			let _ = ensure_signed(origin)?;

			Self::do_close(dao_id, proposal_hash, index, proposal_weight_bound)
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
		meta: BoundedVec<u8, T::ProposalMetadataLimit>,
	) -> Result<(u32, u32), DispatchError> {
		let proposal_len = proposal.encoded_size();

		let proposal_hash = T::Hashing::hash_of(&proposal);
		ensure!(
			!<ProposalOf<T>>::contains_key(dao_id, proposal_hash),
			Error::<T>::DuplicateProposal
		);

		let policy = T::DaoProvider::policy(dao_id)?;

		let active_proposals =
			<Proposals<T>>::try_mutate(dao_id, |proposals| -> Result<usize, DispatchError> {
				proposals.try_push(proposal_hash).map_err(|_| Error::<T>::TooManyProposals)?;
				Ok(proposals.len())
			})?;

		// Make sure using Inline type of Bounded
		let (proposal_dispatch_data, _) = T::Preimages::peek(&proposal)?;

		let index = Self::proposal_count(dao_id);
		<ProposalCount<T>>::mutate(dao_id, |i| *i += 1);
		<ProposalOf<T>>::insert(dao_id, proposal_hash, proposal);
		let votes = {
			let end = frame_system::Pallet::<T>::block_number() + policy.proposal_period.into();
			Votes {
				index,
				threshold,
				ayes: BoundedVec::<AccountVote<T::AccountId, ItemOf<T>>, T::MaxVotes>::default(),
				nays: BoundedVec::<AccountVote<T::AccountId, ItemOf<T>>, T::MaxVotes>::default(),
				end,
			}
		};
		<Voting<T>>::insert(dao_id, proposal_hash, votes);

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

	/// Add an aye or nay vote for the member(by nft in collection) to the given proposal
	pub fn do_vote(
		who: T::AccountId,
		dao_id: DaoId,
		proposal: T::Hash,
		index: ProposalIndex,
		vote: Vote<ItemOf<T>>,
	) -> Result<bool, DispatchError> {
		let mut voting = Self::voting(dao_id, proposal).ok_or(Error::<T>::ProposalMissing)?;
		ensure!(voting.index == index, Error::<T>::WrongIndex);

		let Vote { aye, item } = vote;

		let position_yes = voting.ayes.iter().position(|a| a.vote.item == item);
		let position_no = voting.nays.iter().position(|a| a.vote.item == item);

		// Detects first vote of the member in the motion
		let is_account_voting_first_time = position_yes.is_none() && position_no.is_none();
		if !is_account_voting_first_time {
			return Err(Error::<T>::DuplicateVote.into())
		}

		if aye {
			let mut ayes = voting.ayes.to_vec();
			ayes.push(AccountVote { who: who.clone(), vote: Vote { aye, item } });

			voting.ayes =
				BoundedVec::<AccountVote<T::AccountId, ItemOf<T>>, T::MaxVotes>::try_from(ayes)
					.map_err(|_| Error::<T>::TooManyVotes)?;
		} else {
			let mut nays = voting.nays.to_vec();
			nays.push(AccountVote { who: who.clone(), vote: Vote { aye, item } });

			voting.nays =
				BoundedVec::<AccountVote<T::AccountId, ItemOf<T>>, T::MaxVotes>::try_from(nays)
					.map_err(|_| Error::<T>::TooManyVotes)?;
		}

		Self::deposit_event(Event::Voted {
			dao_id,
			account: who,
			proposal_index: index,
			proposal_hash: proposal,
			vote,
		});

		Voting::<T>::insert(dao_id, proposal, voting);

		// TODO: should we close if approved not waiting till the proposal expires

		Ok(is_account_voting_first_time)
	}

	/// Close a vote that is either approved, disapproved or whose voting period has ended.
	pub fn do_close(
		dao_id: DaoId,
		proposal_hash: T::Hash,
		index: ProposalIndex,
		proposal_weight_bound: Weight,
	) -> DispatchResultWithPostInfo {
		let voting = Self::voting(dao_id, proposal_hash).ok_or(Error::<T>::ProposalMissing)?;
		ensure!(voting.index == index, Error::<T>::WrongIndex);

		let ayes_count: u128 = u128::try_from(voting.ayes.len()).unwrap();
		let nays_count: u128 = u128::try_from(voting.nays.len()).unwrap();
		let turnout = ayes_count.saturating_add(nays_count);
		let sqrt_voters = turnout.integer_sqrt();
		let sqrt_electorate = voting.threshold.integer_sqrt();

		let approved = !sqrt_voters.is_zero() &&
			compare_rationals(nays_count, sqrt_voters, ayes_count, sqrt_electorate);
		let disapproved = false;

		if approved {
			let (proposal, _) =
				Self::validate_and_get_proposal(dao_id, &proposal_hash, proposal_weight_bound)?;
			Self::deposit_event(Event::Closed {
				dao_id,
				proposal_hash,
				proposal_index: index,
				ayes: ayes_count,
				nays: nays_count,
			});
			let (_proposal_weight, _proposal_count) =
				Self::do_approve_proposal(dao_id, index, proposal_hash, proposal);
			return Ok((Some(Weight::from_ref_time(0)), Pays::Yes).into())
		} else if disapproved {
			Self::deposit_event(Event::Closed {
				dao_id,
				proposal_index: index,
				proposal_hash,
				ayes: ayes_count,
				nays: nays_count,
			});

			let _proposal_count = Self::do_disapprove_proposal(dao_id, index, proposal_hash);
			return Ok((Some(Weight::from_ref_time(0)), Pays::No).into())
		}

		// Only allow actual closing of the proposal after the voting period has ended.
		ensure!(frame_system::Pallet::<T>::block_number() >= voting.end, Error::<T>::TooEarly);

		// TODO
		Ok((Some(Weight::from_ref_time(0)), Pays::No).into())
	}

	/// Ensure that the right proposal bounds were passed and get the proposal from storage.
	///
	/// Checks the length in storage via `storage::read` which adds an extra `size_of::<u32>() == 4`
	/// to the length.
	fn validate_and_get_proposal(
		dao_id: DaoId,
		hash: &T::Hash,
		weight_bound: Weight,
	) -> Result<(<T as Config>::Proposal, usize), DispatchError> {
		let key = ProposalOf::<T>::hashed_key_for(dao_id, hash);
		// read the length of the proposal storage entry directly
		let proposal_len =
			storage::read(&key, &mut [0; 0], 0).ok_or(Error::<T>::ProposalMissing)?;
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
		proposal_index: ProposalIndex,
		proposal_hash: T::Hash,
		proposal: <T as Config>::Proposal,
	) -> (Weight, u32) {
		Self::deposit_event(Event::Approved { dao_id, proposal_index, proposal_hash });

		let dao_account_id = T::DaoProvider::dao_account_id(dao_id);

		let dispatch_weight = proposal.get_dispatch_info().weight;
		let origin = RawOrigin::Dao(dao_account_id).into();
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
		ProposalOf::<T>::remove(dao_id, proposal_hash);
		Voting::<T>::remove(dao_id, proposal_hash);
		let num_proposals = Proposals::<T>::mutate(dao_id, |proposals| {
			proposals.retain(|h| h != &proposal_hash);
			proposals.len() + 1 // calculate weight based on original length
		});
		num_proposals as u32
	}
}
