#![cfg_attr(not(feature = "std"), no_std)]
#![recursion_limit = "128"]

use scale_info::{prelude::*, TypeInfo};
use sp_core::{bounded::BoundedVec, Hasher, H160};
use sp_io::storage;
use sp_runtime::{
	traits::{BlockNumberProvider, Hash, IntegerSquareRoot, Zero},
	Saturating,
};
use sp_std::{iter::Sum, prelude::*, str};

use dao_primitives::{
	AccountTokenBalance, DaoPolicy, DaoProvider, DaoToken, EncodeInto, PendingProposal,
	PendingVote, RawOrigin,
};

use sp_io::offchain_index;

// TODO
use pallet_dao_democracy::vote_threshold::compare_rationals;

use frame_support::{
	codec::{Decode, Encode, MaxEncodedLen},
	dispatch::{
		DispatchError, DispatchResultWithPostInfo, Dispatchable, GetDispatchInfo, Pays,
		PostDispatchInfo,
	},
	ensure,
	pallet_prelude::*,
	traits::{
		schedule::{
			v3::{Named as ScheduleNamed, TaskName},
			DispatchTime,
		},
		tokens::Balance,
		Bounded, Get, LockIdentifier, QueryPreimage, StorageVersion, StorePreimage,
	},
	weights::Weight,
	Parameter,
};
use frame_system::{offchain::AppCrypto, pallet_prelude::*};

use eth_primitives::EthRpcProvider;
use serde::Deserialize;

use crate::vote::{AccountVote, Vote};
pub use pallet::*;

pub mod vote;

type BalanceOf<T> = <T as Config>::Balance;

/// Dao ID. Just a `u32`.
pub type DaoId = u32;

/// Token Supply
pub type TokenSupply = u128;

/// Simple index type for proposal counting.
pub type ProposalIndex = u32;

pub type BlockNumber = u32;

pub type BoundedProposal<T> = Bounded<<T as Config>::Proposal>;

pub type CallOf<T> = <T as Config>::RuntimeCall;

const UNSIGNED_TXS_PRIORITY: u64 = 100;

const ONCHAIN_TX_KEY: &[u8] = b"societal-dao-eth-gov::storage::tx";

const DAO_ETH_GOVERNANCE_ID: LockIdentifier = *b"daoethgv";

#[derive(Debug, Encode, Decode, Clone, Default, Deserialize, PartialEq, Eq)]
pub enum OffchainData<Hash, BlockNumber> {
	#[default]
	Default,
	ApproveProposal {
		dao_id: u32,
		token_address: Vec<u8>,
		account_id: Vec<u8>,
		hash: Hash,
		length_bound: u32,
	},
	ApproveVote {
		dao_id: u32,
		token_address: Vec<u8>,
		account_id: Vec<u8>,
		hash: Hash,
		block_number: BlockNumber,
	},
}

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
	/// Particular block number to check eth state at
	block_number: u32,
}

#[derive(Debug, Deserialize, Encode, Decode, Default)]
struct IndexingData<Hash>(Vec<u8>, OffchainData<Hash, BlockNumber>);

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::vote::Vote;
	use eth_primitives::EthRpcService;
	use frame_system::offchain::{CreateSignedTransaction, SubmitTransaction};
	use serde_json::json;
	use sp_runtime::offchain::storage::StorageValueRef;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(4);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + CreateSignedTransaction<Call<Self>> {
		type RuntimeCall: Parameter
			+ Dispatchable<RuntimeOrigin = <Self as Config>::RuntimeOrigin>
			+ From<Call<Self>>
			+ IsType<<Self as frame_system::Config>::RuntimeCall>
			+ From<frame_system::Call<Self>>;

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

		#[pallet::constant]
		type EthRpcUrlLimit: Get<u32>;

		/// Maximum number of proposals allowed to be active in parallel.
		type MaxProposals: Get<ProposalIndex>;

		/// Maximum number of pending proposals allowed to be active in parallel.
		#[pallet::constant]
		type MaxPendingProposals: Get<u32>;

		/// The maximum number of votes(ayes/nays) for a proposal.
		#[pallet::constant]
		type MaxVotes: Get<u32>;

		/// The maximum number of pending votes(ayes/nays) for a proposal.
		#[pallet::constant]
		type MaxPendingVotes: Get<u32>;

		type DaoProvider: DaoProvider<
			<Self as frame_system::Config>::Hash,
			Id = u32,
			AccountId = Self::AccountId,
			Policy = DaoPolicy,
			Origin = OriginFor<Self>,
		>;

		/// The preimage provider with which we look up call hashes to get the call.
		type Preimages: QueryPreimage + StorePreimage;

		/// The identifier type for an offchain worker.
		type AuthorityId: AppCrypto<Self::Public, Self::Signature>;

		type OffchainEthService: EthRpcService;

		type Scheduler: ScheduleNamed<Self::BlockNumber, CallOf<Self>, Self::PalletsOrigin>;

		/// Overarching type of all pallets origins.
		type PalletsOrigin: From<RawOrigin<Self::AccountId>>;
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
		(
			PendingProposal<T::AccountId, BoundedVec<u8, T::ProposalMetadataLimit>, T::BlockNumber>,
			BoundedProposal<T>,
		),
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn pending_proposals)]
	pub(super) type PendingProposals<T: Config> =
		StorageMap<_, Twox64Concat, DaoId, BoundedVec<T::Hash, T::MaxPendingProposals>, ValueQuery>;

	/// Votes on a given proposal, if it is ongoing.
	#[pallet::storage]
	#[pallet::getter(fn voting)]
	pub type Voting<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		DaoId,
		Identity,
		T::Hash,
		Votes<T::BlockNumber, BoundedVec<AccountVote<T::AccountId, BalanceOf<T>>, T::MaxVotes>>,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn pending_votes)]
	pub(super) type PendingVotes<T: Config> = StorageMap<
		_,
		Twox64Concat,
		(DaoId, ProposalIndex),
		BoundedVec<T::Hash, T::MaxPendingVotes>,
		ValueQuery,
	>;

	/// Actual pending voting for a given dao id and hash.
	#[pallet::storage]
	#[pallet::getter(fn pending_voting)]
	pub type PendingVoting<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		DaoId,
		Identity,
		T::Hash,
		PendingVote<T::AccountId, T::Hash, BalanceOf<T>, T::BlockNumber>,
		OptionQuery,
	>;

	/// Proposals so far.
	#[pallet::storage]
	#[pallet::getter(fn proposal_count)]
	pub type ProposalCount<T: Config> = StorageMap<_, Twox64Concat, DaoId, u32, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn eth_rpc_url)]
	pub type EthRpcUrl<T> =
		StorageValue<_, BoundedVec<u8, <T as Config>::EthRpcUrlLimit>, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub eth_rpc_url: Vec<u8>,
		pub _phantom: PhantomData<T>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			GenesisConfig { eth_rpc_url: Default::default(), _phantom: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			EthRpcUrl::<T>::put(
				BoundedVec::<u8, T::EthRpcUrlLimit>::try_from(self.eth_rpc_url.clone()).unwrap(),
			);
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn offchain_worker(block_number: T::BlockNumber) {
			let key = Self::derived_key(block_number);
			let storage_ref = StorageValueRef::persistent(&key);

			if let Ok(Some(data)) = storage_ref.get::<IndexingData<T::Hash>>() {
				log::info!(
					"off-chain indexing data: {:?}, {:?}",
					str::from_utf8(&data.0).unwrap_or("error"),
					data.1
				);

				let offchain_data = data.1;

				let mut call = None;
				match offchain_data.clone() {
					OffchainData::ApproveProposal {
						dao_id,
						token_address,
						account_id,
						hash,
						length_bound: _,
					} => {
						let block_number = match T::OffchainEthService::parse_block_number(
							T::OffchainEthService::fetch_from_eth(
								token_address.clone(),
								Some(json!("eth_blockNumber")),
								None,
								false,
							),
						) {
							Ok(block_number) => block_number,
							Err(e) => {
								log::error!("offchain_worker error: {:?}", e);

								0
							},
						};

						let total_supply = match T::OffchainEthService::parse_token_balance(
							T::OffchainEthService::fetch_token_total_supply(
								token_address.clone(),
								None,
							),
						) {
							Ok(total_supply) => total_supply,
							Err(e) => {
								log::error!("offchain_worker error: {:?}", e);

								0
							},
						};

						match T::OffchainEthService::parse_token_balance(
							T::OffchainEthService::fetch_token_balance_of(
								token_address,
								account_id,
								None,
							),
						) {
							Ok(token_balance) => {
								call = Some(Call::approve_propose {
									dao_id,
									threshold: total_supply,
									block_number,
									hash,
									// TODO: add to DAO settings?
									approve: total_supply > 0 && token_balance > 0,
								});
							},
							Err(e) => {
								log::error!("offchain_worker error: {:?}", e);

								call = Some(Call::approve_propose {
									dao_id,
									threshold: total_supply,
									block_number,
									hash,
									approve: false,
								});
							},
						}
					},
					OffchainData::ApproveVote {
						dao_id,
						token_address,
						account_id,
						hash,
						block_number,
					} => match T::OffchainEthService::parse_token_balance(
						T::OffchainEthService::fetch_token_balance_of(
							token_address,
							account_id,
							Some(block_number),
						),
					) {
						Ok(token_balance) => {
							call = Some(Call::approve_vote {
								dao_id,
								hash,
								approve: token_balance > 0,
							});
						},
						Err(e) => {
							log::error!("offchain_worker error: {:?}", e);

							call = Some(Call::approve_vote { dao_id, hash, approve: false });
						},
					},
					_ => {},
				};

				if call.is_none() {
					return
				}

				let result = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(
					call.unwrap().into(),
				)
				.map_err(|_| {
					log::error!("Failed in offchain_unsigned_tx");
					<Error<T>>::OffchainUnsignedTxError
				});

				if result.is_err() {
					match offchain_data {
						OffchainData::ApproveProposal { dao_id, hash, .. } => {
							log::error!(
								"proposal approval error: dao_id: {:?}, proposal_hash: {:?}",
								dao_id,
								hash
							);
						},
						OffchainData::ApproveVote { dao_id, hash, .. } => {
							log::error!(
								"vote approval error: dao_id: {:?}, proposal_hash: {:?}",
								dao_id,
								hash
							);
						},
						_ => {},
					}
				}
			}
		}
	}

	#[pallet::validate_unsigned]
	impl<T: Config> frame_support::unsigned::ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			let valid_tx = |provide| {
				ValidTransaction::with_tag_prefix("societal-node")
					.priority(UNSIGNED_TXS_PRIORITY)
					.and_provides([&provide])
					.longevity(3)
					.propagate(true)
					.build()
			};

			match call {
				Call::approve_propose { .. } => valid_tx(b"approve_propose".to_vec()),
				Call::approve_vote { .. } => valid_tx(b"approve_vote".to_vec()),
				_ => InvalidTransaction::Call.into(),
			}
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		// Proposal pending approval
		ProposalPendingApproval {
			dao_id: DaoId,
			account: T::AccountId,
			proposal_hash: T::Hash,
			proposal: T::Proposal,
		},
		/// A motion (given hash) has been proposed (by given account) with a threshold
		Proposed {
			dao_id: DaoId,
			account: T::AccountId,
			proposal_index: ProposalIndex,
			proposal_hash: T::Hash,
			proposal: T::Proposal,
			block_number: u32,
			threshold: TokenSupply,
			meta: BoundedVec<u8, T::ProposalMetadataLimit>,
		},
		/// Proposal vote pending approval
		VotePendingApproval {
			dao_id: DaoId,
			account: T::AccountId,
			proposal_hash: T::Hash,
			proposal_index: ProposalIndex,
			vote: Vote<BalanceOf<T>>,
		},
		/// A motion (given hash) has been voted on by given account, leaving
		/// a tally (yes votes and no votes given respectively as `TokenSupply`).
		Voted {
			dao_id: DaoId,
			account: T::AccountId,
			proposal_index: ProposalIndex,
			proposal_hash: T::Hash,
			vote: Vote<BalanceOf<T>>,
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
			ayes: BalanceOf<T>,
			nays: BalanceOf<T>,
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
		/// Action is not allowed for non-eth DAOs
		NotEthDao,
		/// Signer account is not equal to the account provided
		WrongAccountId,
		InvalidInput,
		/// Metadata size exceeds the limits
		MetadataTooLong,
		OffchainUnsignedTxError,
		NotSupported,
		/// Voting is disabled for expired proposals
		Expired,
		TooManyPendingProposals,
		TooManyPendingVotes,
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
			#[pallet::compact] length_bound: u32,
			account_id: Vec<u8>,
		) -> DispatchResultWithPostInfo {
			Self::propose_with_meta(origin, dao_id, proposal, length_bound, account_id, None)
		}

		/// Adds a new proposal with temporary meta field for arbitrary data indexed by node indexer
		#[pallet::weight(10_1000)]
		#[pallet::call_index(1)]
		pub fn propose_with_meta(
			origin: OriginFor<T>,
			dao_id: DaoId,
			proposal: Box<<T as Config>::Proposal>,
			#[pallet::compact] length_bound: u32,
			account_id: Vec<u8>,
			meta: Option<Vec<u8>>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let account_id = Self::validate_account(who.clone(), account_id)?;

			let proposal_hash = T::Hashing::hash_of(&proposal);

			let account_token_balance =
				Self::ensure_proposal_allowed(dao_id, account_id, proposal_hash, length_bound)?;

			let meta = match meta.clone() {
				Some(metadata) => BoundedVec::<u8, T::ProposalMetadataLimit>::try_from(metadata)
					.map_err(|_| Error::<T>::MetadataTooLong)?,
				None => Default::default(),
			};

			match account_token_balance {
				AccountTokenBalance::Offchain { .. } => {
					<PendingProposals<T>>::try_mutate(
						dao_id,
						|hashes| -> Result<usize, DispatchError> {
							hashes
								.try_push(proposal_hash)
								.map_err(|_| Error::<T>::TooManyPendingProposals)?;

							Ok(hashes.len())
						},
					)?;

					let pending_proposal = PendingProposal {
						who: who.clone(),
						length_bound,
						meta,
						block_number: frame_system::Pallet::<T>::block_number(),
					};

					<PendingProposalOf<T>>::insert(
						dao_id,
						proposal_hash,
						(pending_proposal, T::Preimages::bound(proposal.clone())?.transmute()),
					);

					Self::deposit_event(Event::ProposalPendingApproval {
						dao_id,
						account: who,
						proposal_hash,
						proposal: *proposal,
					});

					Ok(Default::default())
				},
				_ => Err(Error::<T>::NotEthDao.into()),
			}
		}

		/// Add an aye or nay vote for the sender to the given proposal.
		#[pallet::weight(10_000)]
		#[pallet::call_index(2)]
		pub fn vote(
			origin: OriginFor<T>,
			dao_id: DaoId,
			proposal: T::Hash,
			#[pallet::compact] index: ProposalIndex,
			vote: Vote<BalanceOf<T>>,
			account_id: Vec<u8>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let account_id = Self::validate_account(who.clone(), account_id)?;

			let voting = Self::ensure_voting(dao_id, proposal, index, who.clone())?;

			// Proposal voting is not allowed if the voting period has ended.
			ensure!(frame_system::Pallet::<T>::block_number() <= voting.end, Error::<T>::Expired);

			let Vote { aye, balance } = vote;

			let pending_vote = PendingVote {
				who: who.clone(),
				proposal_hash: proposal,
				proposal_index: index,
				aye,
				balance,
				block_number: frame_system::Pallet::<T>::block_number(),
			};

			let pending_vote_hash = T::Hashing::hash_of(&pending_vote);

			let account_token_balance = Self::ensure_voting_allowed(
				dao_id,
				account_id,
				pending_vote_hash,
				voting.block_number,
			)?;

			match account_token_balance {
				AccountTokenBalance::Offchain { .. } => {
					<PendingVotes<T>>::try_mutate(
						(dao_id, index),
						|hashes| -> Result<usize, DispatchError> {
							hashes
								.try_push(pending_vote_hash)
								.map_err(|_| Error::<T>::TooManyPendingVotes)?;
							Ok(hashes.len())
						},
					)?;

					<PendingVoting<T>>::insert(dao_id, pending_vote_hash, pending_vote);

					Self::deposit_event(Event::VotePendingApproval {
						dao_id,
						account: who,
						proposal_hash: proposal,
						proposal_index: index,
						vote,
					});

					Ok(Default::default())
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
		/// If the close operation completes successfully with disapproval, the transaction fee will
		/// be waived. Otherwise execution of the approved operation will be charged to the caller.
		///
		/// + `proposal_weight_bound`: The maximum amount of weight consumed by executing the closed
		/// proposal.
		/// + `length_bound`: The upper bound for the length of the proposal in storage. Checked via
		/// `storage::read` so it is `size_of::<u32>() == 4` larger than the pure length.
		#[pallet::weight(10_000)]
		#[pallet::call_index(3)]
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

		#[pallet::weight(10_000)]
		#[pallet::call_index(4)]
		pub fn approve_propose(
			origin: OriginFor<T>,
			dao_id: u32,
			threshold: u128,
			block_number: u32,
			hash: T::Hash,
			approve: bool,
		) -> DispatchResult {
			ensure_none(origin)?;

			Self::do_approve_propose(dao_id, threshold, block_number, hash, approve)?;

			Ok(())
		}

		#[pallet::weight(10_000)]
		#[pallet::call_index(5)]
		pub fn approve_vote(
			origin: OriginFor<T>,
			dao_id: u32,
			hash: T::Hash,
			approve: bool,
		) -> DispatchResult {
			ensure_none(origin)?;

			Self::do_approve_vote(dao_id, hash, approve)?;

			Ok(())
		}

		#[pallet::weight(10_000)]
		#[pallet::call_index(6)]
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

impl<T: Config> Pallet<T> {
	/// Add a new proposal to be voted.
	pub fn do_propose_proposed(
		who: T::AccountId,
		dao_id: DaoId,
		threshold: TokenSupply,
		block_number: u32,
		proposal: BoundedProposal<T>,
		// TODO: remove since bounded is used
		_length_bound: u32,
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
		let now = frame_system::Pallet::<T>::block_number();
		let end = now.saturating_add(policy.proposal_period.into());
		let votes = {
			Votes {
				index,
				threshold,
				ayes: BoundedVec::<AccountVote<T::AccountId, BalanceOf<T>>, T::MaxVotes>::default(),
				nays: BoundedVec::<AccountVote<T::AccountId, BalanceOf<T>>, T::MaxVotes>::default(),
				end,
				block_number,
			}
		};
		<Voting<T>>::insert(dao_id, proposal_hash, votes);

		let account_id = T::DaoProvider::dao_account_id(dao_id);
		let origin = RawOrigin::Dao(account_id).into();

		let call = CallOf::<T>::from(Call::close_scheduled_proposal {
			dao_id,
			proposal_hash,
			index,
			proposal_weight_bound: Default::default(),
			length_bound: Default::default(),
		});

		// Scheduling proposal close
		T::Scheduler::schedule_named(
			(DAO_ETH_GOVERNANCE_ID, dao_id, proposal_hash, index).encode_into(),
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
			block_number,
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
		vote: Vote<BalanceOf<T>>,
	) -> Result<(), DispatchError> {
		let mut voting = Self::ensure_voting(dao_id, proposal, index, who.clone())?;
		ensure!(voting.index == index, Error::<T>::WrongIndex);

		let Vote { aye, balance } = vote;

		if aye {
			let mut ayes = voting.ayes.to_vec();
			ayes.push(AccountVote { who: who.clone(), vote: Vote { aye, balance } });

			voting.ayes =
				BoundedVec::<AccountVote<T::AccountId, BalanceOf<T>>, T::MaxVotes>::try_from(ayes)
					.map_err(|_| Error::<T>::TooManyVotes)?;
		} else {
			let mut nays = voting.nays.to_vec();
			nays.push(AccountVote { who: who.clone(), vote: Vote { aye, balance } });

			voting.nays =
				BoundedVec::<AccountVote<T::AccountId, BalanceOf<T>>, T::MaxVotes>::try_from(nays)
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

		Ok(())
	}

	/// Close a vote that is either approved, disapproved or whose voting period has ended.
	pub fn do_close(
		dao_id: DaoId,
		proposal_hash: T::Hash,
		index: ProposalIndex,
		proposal_weight_bound: Weight,
		length_bound: u32,
	) -> DispatchResultWithPostInfo {
		let voting = Self::voting(dao_id, proposal_hash).ok_or(Error::<T>::ProposalMissing)?;
		ensure!(voting.index == index, Error::<T>::WrongIndex);

		let ayes_balance: BalanceOf<T> =
			voting.ayes.iter().map(|AccountVote { vote, .. }| vote.balance).sum();
		let nays_balance: BalanceOf<T> =
			voting.nays.iter().map(|AccountVote { vote, .. }| vote.balance).sum();
		let turnout = ayes_balance.saturating_add(nays_balance);
		let sqrt_voters = turnout.integer_sqrt();
		let sqrt_electorate = voting.threshold.integer_sqrt();

		if !sqrt_voters.is_zero() &&
			compare_rationals(nays_balance, sqrt_voters, ayes_balance, sqrt_electorate.into())
		{
			let (proposal, _) = Self::validate_and_get_proposal(
				dao_id,
				&proposal_hash,
				length_bound,
				proposal_weight_bound,
			)?;
			Self::deposit_event(Event::Closed {
				dao_id,
				proposal_hash,
				proposal_index: index,
				ayes: ayes_balance,
				nays: nays_balance,
			});
			let (_proposal_weight, _proposal_count) =
				Self::do_approve_proposal(dao_id, index, proposal_hash, proposal);

			Ok((Some(Weight::from_parts(10_000, 0)), Pays::Yes).into())
		} else {
			// Only allow actual closing of the proposal after the voting period has ended.
			ensure!(frame_system::Pallet::<T>::block_number() >= voting.end, Error::<T>::TooEarly);

			Self::deposit_event(Event::Closed {
				dao_id,
				proposal_index: index,
				proposal_hash,
				ayes: ayes_balance,
				nays: nays_balance,
			});

			let _proposal_count = Self::do_disapprove_proposal(dao_id, index, proposal_hash);

			Ok((Some(Weight::from_parts(10_000, 0)), Pays::No).into())
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

	#[deny(clippy::clone_double_ref)]
	pub(crate) fn derived_key(block_number: T::BlockNumber) -> Vec<u8> {
		block_number.using_encoded(|encoded_bn| {
			ONCHAIN_TX_KEY
				.iter()
				.chain(b"/".iter())
				.chain(encoded_bn)
				.copied()
				.collect::<Vec<u8>>()
		})
	}

	/// Validating account_id provided as an argument against call signer
	fn validate_account(
		signer: T::AccountId,
		account_id: Vec<u8>,
	) -> Result<Vec<u8>, DispatchError> {
		let account_id = Result::unwrap_or(str::from_utf8(&account_id), "");
		if account_id.is_empty() {
			return Err(Error::<T>::InvalidInput.into())
		}
		let account_id_stripped = account_id.strip_prefix("0x").unwrap_or(account_id);

		let hex_account = hex::decode(account_id_stripped).map_err(|_| Error::<T>::InvalidInput)?;

		let mut data = [0u8; 24];
		data[0..4].copy_from_slice(b"evm:");
		data[4..24].copy_from_slice(&H160::from_slice(&hex_account[..])[..]);

		let hash = <T::Hashing as Hasher>::hash(&data);
		let acc = T::AccountId::decode(&mut hash.as_ref()).map_err(|_| Error::<T>::InvalidInput)?;

		ensure!(signer == acc, Error::<T>::WrongAccountId);

		Ok(account_id_stripped.as_bytes().to_vec())
	}

	fn do_approve_propose(
		dao_id: DaoId,
		threshold: TokenSupply,
		block_number: u32,
		hash: T::Hash,
		approve: bool,
	) -> Result<(), DispatchError> {
		let (pending_proposal, proposal) =
			<PendingProposalOf<T>>::take(dao_id, hash).expect("Pending Proposal not found");

		let PendingProposal { who, length_bound, meta, .. } = pending_proposal;

		PendingProposals::<T>::mutate(dao_id, |hashes| {
			hashes.retain(|h| h != &hash);
			hashes.len() + 1
		});

		if approve {
			Self::do_propose_proposed(
				who,
				dao_id,
				threshold,
				block_number,
				proposal,
				length_bound,
				meta,
			)?;
		}

		Ok(())
	}

	fn do_approve_vote(dao_id: DaoId, hash: T::Hash, approve: bool) -> Result<(), DispatchError> {
		let PendingVote { who, proposal_hash, proposal_index, aye, balance, .. } =
			<PendingVoting<T>>::take(dao_id, hash).expect("Pending Vote not found");

		PendingVotes::<T>::mutate((dao_id, proposal_index), |hashes| {
			hashes.retain(|h| h != &hash);
			hashes.len() + 1
		});

		if approve {
			Self::do_vote(who, dao_id, proposal_hash, proposal_index, Vote { aye, balance })?;
		}

		Ok(())
	}

	fn ensure_proposal_allowed(
		id: DaoId,
		account_id: Vec<u8>,
		hash: T::Hash,
		length_bound: u32,
	) -> Result<AccountTokenBalance, DispatchError> {
		let token_balance = Self::ensure_token_balance(id)?;

		if let AccountTokenBalance::Offchain { token_address } = token_balance.clone() {
			let key = Self::derived_key(frame_system::Pallet::<T>::block_number());

			let data: IndexingData<T::Hash> = IndexingData(
				b"approve_propose".to_vec(),
				OffchainData::ApproveProposal {
					dao_id: id,
					token_address,
					account_id,
					hash,
					length_bound,
				},
			);

			offchain_index::set(&key, &data.encode());
		}

		Ok(token_balance)
	}

	fn ensure_voting_allowed(
		id: DaoId,
		account_id: Vec<u8>,
		hash: T::Hash,
		block_number: u32,
	) -> Result<AccountTokenBalance, DispatchError> {
		let token_balance = Self::ensure_token_balance(id)?;

		if let AccountTokenBalance::Offchain { token_address } = token_balance.clone() {
			let key = Self::derived_key(frame_system::Pallet::<T>::block_number());

			let data: IndexingData<T::Hash> = IndexingData(
				b"approve_proposal_vote".to_vec(),
				OffchainData::ApproveVote {
					dao_id: id,
					token_address,
					account_id,
					hash,
					block_number,
				},
			);

			offchain_index::set(&key, &data.encode());
		}

		Ok(token_balance)
	}

	fn ensure_token_balance(id: DaoId) -> Result<AccountTokenBalance, DispatchError> {
		let dao_token = T::DaoProvider::dao_token(id)?;

		match dao_token {
			DaoToken::FungibleToken(_) => Err(Error::<T>::NotSupported.into()),
			DaoToken::EthTokenAddress(token_address) =>
				Ok(AccountTokenBalance::Offchain { token_address: token_address.to_vec() }),
		}
	}

	fn ensure_voting(
		dao_id: DaoId,
		proposal: T::Hash,
		index: ProposalIndex,
		who: T::AccountId,
	) -> Result<
		Votes<T::BlockNumber, BoundedVec<AccountVote<T::AccountId, BalanceOf<T>>, T::MaxVotes>>,
		DispatchError,
	> {
		let voting = Self::voting(dao_id, proposal).ok_or(Error::<T>::ProposalMissing)?;
		ensure!(voting.index == index, Error::<T>::WrongIndex);

		let position_yes = voting.ayes.iter().position(|a| a.who == who);
		let position_no = voting.nays.iter().position(|a| a.who == who);
		let is_account_voting_first_time = position_yes.is_none() && position_no.is_none();

		if !is_account_voting_first_time {
			return Err(Error::<T>::DuplicateVote.into())
		}

		Ok(voting)
	}

	fn close_proposal_task_id(
		dao_id: DaoId,
		proposal_hash: T::Hash,
		index: ProposalIndex,
	) -> TaskName {
		(DAO_ETH_GOVERNANCE_ID, dao_id, proposal_hash, index).encode_into()
	}
}

impl<T: Config> BlockNumberProvider for Pallet<T> {
	type BlockNumber = T::BlockNumber;
	fn current_block_number() -> Self::BlockNumber {
		<frame_system::Pallet<T>>::block_number()
	}
}

impl<T: Config> EthRpcProvider for Pallet<T> {
	fn get_rpc_url() -> Vec<u8> {
		EthRpcUrl::<T>::get().to_vec()
	}
}
