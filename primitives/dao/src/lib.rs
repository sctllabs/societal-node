#![cfg_attr(not(feature = "std"), no_std)]

use codec::MaxEncodedLen;
#[cfg(feature = "runtime-benchmarks")]
use frame_support::dispatch::DispatchResultWithPostInfo;
use frame_support::{
	codec::{Decode, Encode},
	dispatch::{DispatchError, DispatchResult},
	weights::Weight,
};
pub use node_primitives::Balance;

use scale_info::TypeInfo;
use serde::{self, Deserialize, Deserializer, Serialize};
use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;

pub type DispatchResultWithDaoOrigin<T> = Result<DaoOrigin<T>, DispatchError>;

// TODO: retrieve from runtime
pub const EXPECTED_BLOCK_TIME: u32 = 12; // in seconds

pub const DAY_IN_BLOCKS: u32 = 24 * 60 * 60 / EXPECTED_BLOCK_TIME;
pub const MONTH_IN_BLOCKS: u32 = 30 * DAY_IN_BLOCKS;

pub const DEFAULT_SUBSCRIPTION_PRICE: u128 = 1_000_000_000_000_000;
pub const DEFAULT_FUNCTION_CALL_LIMIT: u32 = 10000;
pub const DEFAULT_FUNCTION_PER_BLOCK_LIMIT: u32 = 100;
pub const DEFAULT_MEMBER_COUNT_LIMIT: u32 = 100;

#[derive(
	Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, Serialize, Deserialize, MaxEncodedLen,
)]
pub struct BountyPayoutDelay(pub u32);
impl Default for BountyPayoutDelay {
	fn default() -> Self {
		BountyPayoutDelay(DAY_IN_BLOCKS)
	}
}

#[derive(
	Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, Serialize, Deserialize, MaxEncodedLen,
)]
pub struct BountyUpdatePeriod(pub u32);
impl Default for BountyUpdatePeriod {
	fn default() -> Self {
		BountyUpdatePeriod(DAY_IN_BLOCKS)
	}
}

#[derive(
	Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, Serialize, Deserialize, MaxEncodedLen,
)]
pub struct TreasurySpendPeriod(pub u32);
impl Default for TreasurySpendPeriod {
	fn default() -> Self {
		TreasurySpendPeriod(DAY_IN_BLOCKS)
	}
}

#[derive(
	Encode, Decode, Default, Clone, PartialEq, TypeInfo, RuntimeDebug, Serialize, Deserialize,
)]
pub struct DaoTokenMetadata {
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub name: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub symbol: Vec<u8>,
	pub decimals: u8,
}

#[derive(
	Encode, Decode, Default, Clone, PartialEq, TypeInfo, RuntimeDebug, Serialize, Deserialize,
)]
pub struct DaoGovernanceToken {
	pub token_id: u128,
	pub metadata: DaoTokenMetadata,
	#[serde(default)]
	#[serde(deserialize_with = "de_option_string_to_u128")]
	pub min_balance: Option<u128>,
	#[serde(default)]
	#[serde(deserialize_with = "de_option_string_to_u128")]
	pub initial_balance: Option<u128>,
}

#[derive(Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, Serialize, Deserialize)]
pub struct DaoPayload {
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub name: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub purpose: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub metadata: Vec<u8>,
	pub token: Option<DaoGovernanceToken>,
	pub token_id: Option<u128>,
	#[serde(default)]
	#[serde(deserialize_with = "de_option_string_to_bytes")]
	pub token_address: Option<Vec<u8>>,
	pub policy: DaoPolicy,
	pub tier: Option<VersionedDaoSubscriptionTier>,
}

#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct DaoConfig<BoundedName, BoundedString, BoundedMetadata> {
	/// Name of the DAO.
	pub name: BoundedName,
	/// Purpose of this DAO.
	pub purpose: BoundedString,
	/// Generic metadata. Can be used to store additional data.
	pub metadata: BoundedMetadata,
}

#[derive(
	Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, Serialize, Deserialize, MaxEncodedLen,
)]
pub struct DaoPolicy {
	/// In blocks
	pub proposal_period: u32,
	#[serde(default)]
	pub approve_origin: DaoPolicyProportion,
	/// Governance settings
	#[serde(default)]
	pub governance: Option<DaoGovernance>,
	/// The delay period for which a bounty beneficiary need to wait before claim the payout.
	/// By default - 1 day. Considering the block time is set to 6 seconds - .
	#[serde(default)]
	pub bounty_payout_delay: BountyPayoutDelay,
	/// Bounty duration in blocks.
	/// Same as payout delay by default.
	#[serde(default)]
	pub bounty_update_period: BountyUpdatePeriod,
	/// Periodic treasury spend period in blocks
	#[serde(default)]
	pub spend_period: TreasurySpendPeriod,
}

#[derive(
	Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, Serialize, Deserialize, MaxEncodedLen,
)]
pub struct GovernanceV1Policy {
	/// The period between a proposal being approved and enacted.
	///
	/// It should generally be a little more than the unstake period to ensure that
	/// voting stakers have an opportunity to remove themselves from the system in the case
	/// where they are on the losing side of a vote.
	pub enactment_period: u32,
	/// How often (in blocks) new public referenda are launched.
	pub launch_period: u32,
	/// How often (in blocks) to check for new votes.
	pub voting_period: u32,
	/// The minimum period of vote locking.
	///
	/// It should be no shorter than enactment period to ensure that in the case of an
	/// approval, those successful voters are locked into the consequences that their votes
	/// entail. Same as EnactmentPeriod
	pub vote_locking_period: u32,
	/// Minimum voting period allowed for a fast-track referendum.
	pub fast_track_voting_period: u32,
	/// Period in blocks where an external proposal may not be re-submitted after being vetoed.
	pub cooloff_period: u32,
	/// The minimum amount to be used as a deposit for a public referendum proposal.
	pub minimum_deposit: u128,
	/// Origin from which the next tabled referendum may be forced. This is a normal
	/// "super-majority-required" referendum.
	#[serde(default)]
	pub external_origin: DaoPolicyProportion,
	/// Origin from which the next tabled referendum may be forced; this allows for the tabling
	/// of a majority-carries referendum.
	#[serde(default)]
	pub external_majority_origin: DaoPolicyProportion,
	/// Origin from which the next tabled referendum may be forced; this allows for the tabling
	/// of a negative-turnout-bias (default-carries) referendum.
	#[serde(default)]
	pub external_default_origin: DaoPolicyProportion,
	/// Origin from which the next majority-carries (or more permissive) referendum may be
	/// tabled to vote according to the `FastTrackVotingPeriod` asynchronously in a similar
	/// manner to the emergency origin. It retains its threshold method.
	#[serde(default)]
	pub fast_track_origin: DaoPolicyProportion,
	/// Origin from which the next majority-carries (or more permissive) referendum may be
	/// tabled to vote immediately and asynchronously in a similar manner to the emergency
	/// origin. It retains its threshold method.
	#[serde(default)]
	pub instant_origin: DaoPolicyProportion,
	/// Indicator for whether an emergency origin is even allowed to happen. Some chains may
	/// want to set this permanently to `false`, others may want to condition it on things such
	/// as an upgrade having happened recently.
	#[serde(default)]
	pub instant_allowed: bool,
	/// Origin from which any referendum may be cancelled in an emergency.
	#[serde(default)]
	pub cancellation_origin: DaoPolicyProportion,
	/// Origin from which proposals may be blacklisted.
	#[serde(default)]
	pub blacklist_origin: DaoPolicyProportion,
	/// Origin from which a proposal may be cancelled and its backers slashed.
	#[serde(default)]
	pub cancel_proposal_origin: DaoPolicyProportion,
}

#[derive(
	Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, Serialize, Deserialize, MaxEncodedLen,
)]
pub enum DaoGovernance {
	GovernanceV1(GovernanceV1Policy),
	OwnershipWeightedVoting,
}

#[derive(
	Encode, Decode, Copy, Clone, Default, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen,
)]
pub enum DaoStatus {
	// Pending approval from off-chain
	#[default]
	Pending,
	// DAO approved on-chain
	Success,
	// An error occurred while approving DAO
	Error,
}

#[derive(Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum DaoToken<TokenId, BoundedString> {
	FungibleToken(TokenId),
	EthTokenAddress(BoundedString),
}

#[derive(Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct Dao<AccountId, TokenId, BoundedName, BoundedString, BoundedMetadata> {
	pub founder: AccountId,
	pub account_id: AccountId,
	pub token: DaoToken<TokenId, BoundedString>,
	pub status: DaoStatus,
	pub config: DaoConfig<BoundedName, BoundedString, BoundedMetadata>,
}

#[derive(Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct PendingDao<
	AccountId,
	TokenId,
	BoundedName,
	BoundedString,
	BoundedMetadata,
	BoundedCouncilMembers,
	BoundedTechnicalCommittee,
	BlockNumber,
> {
	pub dao: Dao<AccountId, TokenId, BoundedName, BoundedString, BoundedMetadata>,
	pub policy: DaoPolicy,
	pub council: BoundedCouncilMembers,
	pub technical_committee: BoundedTechnicalCommittee,
	pub block_number: BlockNumber,
}

#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct PendingProposal<AccountId, BoundedMetadata, BlockNumber> {
	pub who: AccountId,
	pub length_bound: u32,
	pub meta: BoundedMetadata,
	pub block_number: BlockNumber,
}

#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct PendingVote<AccountId, Hash, Balance, BlockNumber> {
	pub who: AccountId,
	pub proposal_hash: Hash,
	pub proposal_index: u32,
	pub aye: bool,
	pub balance: Balance,
	pub block_number: BlockNumber,
}

#[derive(
	Encode,
	Decode,
	Default,
	Clone,
	PartialEq,
	Eq,
	TypeInfo,
	RuntimeDebug,
	MaxEncodedLen,
	Deserialize,
)]
pub struct DaoApprovalPayload<DaoId, BoundedString> {
	pub dao_id: DaoId,
	pub token_address: BoundedString,
}

#[derive(Clone, Default, PartialEq, TypeInfo, RuntimeDebug)]
pub enum AccountTokenBalance {
	#[default]
	Insufficient,
	Sufficient,
	// An off-chain action should be performed
	Offchain {
		token_address: Vec<u8>,
	},
}

pub type Proportion = (u32, u32);

#[derive(
	Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, Serialize, Deserialize, MaxEncodedLen,
)]
#[serde(tag = "type", content = "proportion")]
pub enum DaoPolicyProportion {
	AtLeast(Proportion),
	MoreThan(Proportion),
}

impl Default for DaoPolicyProportion {
	fn default() -> Self {
		Self::AtLeast((1, 2))
	}
}

pub trait DaoProvider<AccountId, Hash> {
	type Id;
	type AssetId;
	type Policy;
	type Origin;
	type ApproveOrigin;
	type NFTCollectionId;

	fn exists(id: Self::Id) -> Result<(), DispatchError>;
	fn dao_account_id(id: Self::Id) -> AccountId;
	fn dao_token(id: Self::Id) -> Result<DaoToken<Self::AssetId, Vec<u8>>, DispatchError>;
	fn policy(id: Self::Id) -> Result<Self::Policy, DispatchError>;
	fn dao_nft_collection_id(id: Self::Id) -> Result<Option<Self::NFTCollectionId>, DispatchError>;
	fn count() -> u32;
	fn ensure_member(id: Self::Id, who: &AccountId) -> Result<bool, DispatchError>;
	fn ensure_approved(
		origin: Self::Origin,
		dao_id: Self::Id,
	) -> DispatchResultWithDaoOrigin<AccountId>;

	/// Note: Should only be used for benchmarking.
	#[cfg(feature = "runtime-benchmarks")]
	fn create_dao(
		founder: AccountId,
		council: Vec<AccountId>,
		technical_committee: Vec<AccountId>,
		data: Vec<u8>,
	) -> Result<(), DispatchError>;

	/// Note: Should only be used for benchmarking.
	#[cfg(feature = "runtime-benchmarks")]
	fn approve_dao(dao_hash: Hash, approve: bool) -> Result<(), DispatchError>;

	/// Note: Should only be used for benchmarking.
	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin(dao_origin: &DaoOrigin<AccountId>) -> Result<Self::Origin, ()>;
}

pub trait InitializeDaoMembers<DaoId, AccountId> {
	fn initialize_members(dao_id: DaoId, members: Vec<AccountId>) -> Result<(), DispatchError>;
}

pub trait ContainsDaoMember<DaoId, AccountId> {
	fn contains(dao_id: DaoId, who: &AccountId) -> Result<bool, DispatchError>;
}

pub trait RemoveDaoMembers<DaoId> {
	fn remove_members(dao_id: DaoId, purge: bool) -> Result<(), DispatchError>;
}

/// Trait for type that can handle incremental changes to a set of account IDs.
pub trait ChangeDaoMembers<DaoId, AccountId: Clone + Ord> {
	/// A number of members `incoming` just joined the set and replaced some `outgoing` ones. The
	/// new set is given by `new`, and need not be sorted.
	fn change_members(
		dao_id: DaoId,
		incoming: &[AccountId],
		outgoing: &[AccountId],
		mut new: Vec<AccountId>,
	) {
		new.sort();
		Self::change_members_sorted(dao_id, incoming, outgoing, &new[..]);
	}

	/// A number of members `_incoming` just joined the set and replaced some `_outgoing` ones. The
	/// new set is thus given by `sorted_new` and **must be sorted**.
	///
	/// NOTE: This is the only function that needs to be implemented in `ChangeDaoMembers`.
	fn change_members_sorted(
		dao_id: DaoId,
		incoming: &[AccountId],
		outgoing: &[AccountId],
		sorted_new: &[AccountId],
	);

	/// Set the new members; they **must already be sorted**. This will compute the diff and use it
	/// to call `change_members_sorted`.
	fn set_members_sorted(dao_id: DaoId, new_members: &[AccountId], old_members: &[AccountId]) {
		let (incoming, outgoing) = Self::compute_members_diff_sorted(new_members, old_members);
		Self::change_members_sorted(dao_id, &incoming[..], &outgoing[..], new_members);
	}

	/// Compute diff between new and old members; they **must already be sorted**.
	///
	/// Returns incoming and outgoing members.
	fn compute_members_diff_sorted(
		new_members: &[AccountId],
		old_members: &[AccountId],
	) -> (Vec<AccountId>, Vec<AccountId>) {
		let mut old_iter = old_members.iter();
		let mut new_iter = new_members.iter();
		let mut incoming = Vec::new();
		let mut outgoing = Vec::new();
		let mut old_i = old_iter.next();
		let mut new_i = new_iter.next();
		loop {
			match (old_i, new_i) {
				(None, None) => break,
				(Some(old), Some(new)) if old == new => {
					old_i = old_iter.next();
					new_i = new_iter.next();
				},
				(Some(old), Some(new)) if old < new => {
					outgoing.push(old.clone());
					old_i = old_iter.next();
				},
				(Some(old), None) => {
					outgoing.push(old.clone());
					old_i = old_iter.next();
				},
				(_, Some(new)) => {
					incoming.push(new.clone());
					new_i = new_iter.next();
				},
			}
		}
		(incoming, outgoing)
	}
}

pub trait SpendDaoFunds<DaoId> {
	fn spend_dao_funds(dao_id: DaoId) -> Weight;
}

/// Empty implementation in case no callbacks are required.
impl<DaoId> SpendDaoFunds<DaoId> for () {
	fn spend_dao_funds(_dao_id: DaoId) -> Weight {
		Weight::zero()
	}
}

pub trait DaoReferendumScheduler<DaoId> {
	fn launch_referendum(dao_id: DaoId) -> DispatchResult;
	fn bake_referendum(dao_id: DaoId) -> DispatchResult;
}

/// Empty implementation.
impl<DaoId> DaoReferendumScheduler<DaoId> for () {
	fn launch_referendum(_dao_id: DaoId) -> DispatchResult {
		Ok(())
	}
	fn bake_referendum(_dao_id: DaoId) -> DispatchResult {
		Ok(())
	}
}

pub trait DaoDemocracyProvider<DaoId> {
	fn purge_dao(dao_id: DaoId) -> DispatchResult;
}

/// Empty implementation.
impl<DaoId> DaoDemocracyProvider<DaoId> for () {
	fn purge_dao(_: DaoId) -> DispatchResult {
		Ok(())
	}
}

pub trait DaoEthGovernanceProvider<DaoId> {
	fn purge_dao(dao_id: DaoId) -> DispatchResult;
}

/// Empty implementation.
impl<DaoId> DaoEthGovernanceProvider<DaoId> for () {
	fn purge_dao(_: DaoId) -> DispatchResult {
		Ok(())
	}
}

pub trait DaoBountiesProvider<DaoId> {
	fn purge_dao(dao_id: DaoId) -> DispatchResult;
}

/// Empty implementation.
impl<DaoId> DaoBountiesProvider<DaoId> for () {
	fn purge_dao(_: DaoId) -> DispatchResult {
		Ok(())
	}
}

#[cfg(feature = "runtime-benchmarks")]
pub trait DaoReferendumBenchmarkHelper<DaoId, AccountId, Proposal, Balance> {
	fn propose(
		who: AccountId,
		dao_id: DaoId,
		proposal: Proposal,
		value: Balance,
	) -> DispatchResultWithPostInfo;
}

/// Empty implementation.
#[cfg(feature = "runtime-benchmarks")]
impl<DaoId, AccountId, Proposal, Balance>
	DaoReferendumBenchmarkHelper<DaoId, AccountId, Proposal, Balance> for ()
{
	fn propose(
		_who: AccountId,
		_dao_id: DaoId,
		_proposal: Proposal,
		_value: Balance,
	) -> DispatchResultWithPostInfo {
		Ok(().into())
	}
}

/// Origin for the collective module.
#[derive(PartialEq, Eq, Clone, RuntimeDebug, Encode, Decode, TypeInfo, MaxEncodedLen)]
#[codec(mel_bound(AccountId: MaxEncodedLen))]
pub enum RawOrigin<AccountId> {
	Dao(AccountId),
}

#[derive(Clone)]
pub struct DaoOrigin<AccountId> {
	pub dao_account_id: AccountId,
	pub proportion: DaoPolicyProportion,
}

pub fn de_string_to_bytes<'de, D>(de: D) -> Result<Vec<u8>, D::Error>
where
	D: Deserializer<'de>,
{
	let s: &str = Deserialize::deserialize(de)?;
	Ok(s.as_bytes().to_vec())
}

pub fn de_option_string_to_bytes<'de, D>(de: D) -> Result<Option<Vec<u8>>, D::Error>
where
	D: Deserializer<'de>,
{
	match Option::<&str>::deserialize(de)? {
		None => Ok(None),
		Some(s) => Ok(Some(s.as_bytes().to_vec())),
	}
}

pub fn de_string_to_u128<'de, D>(de: D) -> Result<u128, D::Error>
where
	D: Deserializer<'de>,
{
	let s: &str = Deserialize::deserialize(de)?;
	Ok(s.parse::<u128>().unwrap())
}

pub fn de_option_string_to_u128<'de, D>(de: D) -> Result<Option<u128>, D::Error>
where
	D: Deserializer<'de>,
{
	match Option::<&str>::deserialize(de)? {
		None => Ok(None),
		Some(s) => Ok(Some(s.parse::<u128>().unwrap())),
	}
}

pub struct SchedulerEncoded {}

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

#[derive(Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum SuspensionReason {
	BadActor,
	NotRenewed,
	Other,
}

#[derive(Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub enum DaoSubscriptionStatus<BlockNumber> {
	Active { until: BlockNumber },
	Suspended { at: BlockNumber, reason: SuspensionReason },
}

pub type DaoFunctionBalance = u32;

pub type FunctionPerBlock<BlockNumber, FunctionBalance> = (BlockNumber, FunctionBalance);

pub type DaoMembersCount = u32;

#[derive(Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct DaoSubscription<BlockNumber, Tier, Details> {
	pub tier: Tier,
	pub details: Details,
	pub subscribed_at: BlockNumber,
	pub last_renewed_at: Option<BlockNumber>,
	pub status: DaoSubscriptionStatus<BlockNumber>,
	pub fn_balance: DaoFunctionBalance,
	pub fn_per_block: FunctionPerBlock<BlockNumber, DaoFunctionBalance>,
}

pub trait DaoSubscriptionProvider<DaoId, AccountId, BlockNumber, Tier, Details> {
	/// Subscribes DAO
	/// - `dao_id`: DAO ID.
	/// - `account_id`: The Account to charge the initial subscription payment from.
	/// - `tier`: Subscription tier to subscribe to. Optional.
	fn subscribe(
		dao_id: DaoId,
		account_id: &AccountId,
		tier: Option<Tier>,
	) -> Result<(), DispatchError>;

	/// Unsubscribes DAO
	/// - `dao_id`: DAO ID.
	fn unsubscribe(dao_id: DaoId) -> Result<(), DispatchError>;

	/// Extends DAO subscription
	/// - `dao_id`: DAO ID.
	/// - `account_id`: The Account to charge the subscription payment from.
	fn extend_subscription(dao_id: DaoId, account_id: &AccountId) -> Result<(), DispatchError>;

	/// Extends DAO subscription
	/// - `dao_id`: DAO ID.
	/// - `account_id`: The Account to charge the subscription payment from.
	/// - `tier`: Subscription tier to switch to.
	fn change_subscription_tier(
		dao_id: DaoId,
		account_id: &AccountId,
		tier: Tier,
	) -> Result<(), DispatchError>;

	/// Note: Should only be used for benchmarking.
	#[cfg(feature = "runtime-benchmarks")]
	fn assign_subscription_tier(tier: VersionedDaoSubscriptionTier) -> DispatchResult;
}

/// Empty implementation.
impl<DaoId, AccountId, BlockNumber, Balance>
	DaoSubscriptionProvider<
		DaoId,
		AccountId,
		BlockNumber,
		VersionedDaoSubscriptionTier,
		VersionedDaoSubscriptionDetails<BlockNumber, Balance>,
	> for ()
{
	fn subscribe(
		_dao_id: DaoId,
		_account_id: &AccountId,
		_tier: Option<VersionedDaoSubscriptionTier>,
	) -> Result<(), DispatchError> {
		Ok(())
	}

	fn unsubscribe(_: DaoId) -> Result<(), DispatchError> {
		Ok(())
	}

	fn extend_subscription(_dao_id: DaoId, _account_id: &AccountId) -> Result<(), DispatchError> {
		Ok(())
	}

	fn change_subscription_tier(
		_dao_id: DaoId,
		_account_id: &AccountId,
		_tier: VersionedDaoSubscriptionTier,
	) -> Result<(), DispatchError> {
		Ok(())
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn assign_subscription_tier(_tier: VersionedDaoSubscriptionTier) -> DispatchResult {
		todo!()
	}
}

#[derive(
	Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen, Serialize, Deserialize,
)]
pub struct DaoPalletSubscriptionDetailsV1 {
	pub update_dao_metadata: bool,
	pub update_dao_policy: bool,
	pub mint_dao_token: bool,
}

#[derive(
	Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen, Serialize, Deserialize,
)]
pub struct BountiesSubscriptionDetailsV1 {
	pub create_bounty: bool,
	pub propose_curator: bool,
	pub unassign_curator: bool,
	pub accept_curator: bool,
	pub award_bounty: bool,
	pub claim_bounty: bool,
	pub close_bounty: bool,
	pub extend_bounty_expiry: bool,
}

#[derive(
	Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen, Serialize, Deserialize,
)]
pub struct CollectiveSubscriptionDetailsV1 {
	pub propose: bool,
	pub vote: bool,
	pub close: bool,
}

#[derive(
	Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen, Serialize, Deserialize,
)]
pub struct DemocracySubscriptionDetailsV1 {
	pub propose: bool,
	pub second: bool,
	pub vote: bool,
	pub delegate: bool,
	pub undelegate: bool,
	pub unlock: bool,
	pub remove_vote: bool,
	pub remove_other_vote: bool,
}

#[derive(
	Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen, Serialize, Deserialize,
)]
pub struct MembershipSubscriptionDetailsV1 {
	pub add_member: bool,
	pub remove_member: bool,
	pub swap_member: bool,
	pub change_key: bool,
}

#[derive(
	Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen, Serialize, Deserialize,
)]
pub struct TreasurySubscriptionDetailsV1 {
	pub spend: bool,
	pub transfer_token: bool,
}

#[derive(
	Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen, Serialize, Deserialize,
)]
pub struct DaoSubscriptionDetailsV1<BlockNumber, Balance> {
	pub duration: BlockNumber,
	pub price: Balance,
	pub fn_call_limit: DaoFunctionBalance,
	pub fn_per_block_limit: DaoFunctionBalance,
	pub max_members: DaoMembersCount,
	pub dao: Option<DaoPalletSubscriptionDetailsV1>,
	pub bounties: Option<BountiesSubscriptionDetailsV1>,
	pub council: Option<CollectiveSubscriptionDetailsV1>,
	pub tech_committee: Option<CollectiveSubscriptionDetailsV1>,
	pub democracy: Option<DemocracySubscriptionDetailsV1>,
	pub council_membership: Option<MembershipSubscriptionDetailsV1>,
	pub tech_committee_membership: Option<MembershipSubscriptionDetailsV1>,
	pub treasury: Option<TreasurySubscriptionDetailsV1>,
}

#[derive(
	Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen, Serialize, Deserialize,
)]
pub enum VersionedDaoSubscriptionDetails<BlockNumber, Balance> {
	Default(DaoSubscriptionDetailsV1<BlockNumber, Balance>),
}

#[derive(
	Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen, Serialize, Deserialize,
)]
pub enum DaoSubscriptionTierV1 {
	Basic,
	Standard,
	Premium,

	/// Note: Should only be used for benchmarking.
	#[cfg(feature = "runtime-benchmarks")]
	NoTier,
}

#[derive(
	Encode, Decode, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen, Serialize, Deserialize,
)]
pub enum VersionedDaoSubscriptionTier {
	Default(DaoSubscriptionTierV1),
}

#[repr(u8)]
pub enum DaoTxValidityError {
	/// Unknown
	Unknown = 0,
	/// Not DAO Member
	NotDaoMember = 1,
	/// Subscription Error
	DaoSubscriptionError = 2,
	/// Too many calls for the account per block
	AccountRateLimitExceeded = 3,
	/// Subscription Tier Error
	SubscriptionTierError = 4,
	/// Function call disabled
	FunctionDisabled = 5,
	/// Too many members
	TooManyMembers = 6,
}

impl From<DaoTxValidityError> for u8 {
	fn from(err: DaoTxValidityError) -> Self {
		err as u8
	}
}

/// Account Function Call Rate Limiter
pub trait AccountFnCallRateLimiter<AccountId, FnCallLimit> {
	/// Ensures whether Account ID function call is within block limits
	/// - `account_id`: The Account to check the rate limit for.
	fn ensure_limited(account_id: &AccountId, limit: FnCallLimit) -> Result<(), DispatchError>;
}
