#![cfg_attr(not(feature = "std"), no_std)]

use codec::MaxEncodedLen;
use frame_support::{
	codec::{Decode, Encode},
	dispatch::DispatchError,
};
pub use node_primitives::Balance;

use scale_info::TypeInfo;
use serde::{self, Deserialize, Deserializer, Serialize};
use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;

#[derive(
	Encode, Decode, Default, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, Serialize, Deserialize,
)]
pub struct DaoTokenMetadata {
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub name: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub symbol: Vec<u8>,
	pub decimals: u8,
}

#[derive(
	Encode, Decode, Default, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, Serialize, Deserialize,
)]
pub struct DaoGovernanceToken {
	pub token_id: u32,
	pub metadata: DaoTokenMetadata,
	#[serde(deserialize_with = "de_string_to_u128")]
	pub min_balance: u128,
}

#[derive(
	Encode, Decode, Default, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, Serialize, Deserialize,
)]
pub struct DaoPolicyPayload {
	pub proposal_bond: u32,
	pub proposal_bond_min: u128,
	pub proposal_period: u32,
	pub approve_origin: (u32, u32),
	pub reject_origin: (u32, u32),
}

#[derive(
	Encode, Decode, Default, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, Serialize, Deserialize,
)]
pub struct DaoPayload {
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub name: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub purpose: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub metadata: Vec<u8>,
	pub token: Option<DaoGovernanceToken>,
	pub token_id: Option<u32>,
	#[serde(default)]
	#[serde(deserialize_with = "de_option_string_to_bytes")]
	pub token_address: Option<Vec<u8>>,
	pub policy: DaoPolicyPayload,
}

#[derive(Encode, Decode, Default, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct DaoConfig<BoundedString, BoundedMetadata> {
	/// Name of the DAO.
	pub name: BoundedString,
	/// Purpose of this DAO.
	pub purpose: BoundedString,
	/// Generic metadata. Can be used to store additional data.
	pub metadata: BoundedMetadata,
}

// TODO: replace with payload directly
#[derive(
	Encode,
	Decode,
	Default,
	Clone,
	PartialEq,
	Eq,
	TypeInfo,
	RuntimeDebug,
	Serialize,
	Deserialize,
	MaxEncodedLen,
)]
pub struct DaoPolicy {
	/// Fraction of a proposal's value that should be bonded in order to place the proposal.
	/// An accepted proposal gets these back. A rejected proposal does not.
	pub proposal_bond: u32, //TODO: static value or percentage???
	/// Minimum amount of funds that should be placed in a deposit for making a proposal.
	pub proposal_bond_min: u128,
	/// Maximum amount of funds that should be placed in a deposit for making a proposal.
	pub proposal_bond_max: Option<u128>,
	/// In millis
	pub proposal_period: u32,
	// TODO: use max members for account length
	pub approve_origin: (u32, u32),
	pub reject_origin: (u32, u32),
	pub token_voting_min_threshold: u128,
}

// TODO: add token enum

#[derive(
	Encode, Decode, Copy, Clone, Default, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen,
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

#[derive(Encode, Decode, Default, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct Dao<AccountId, TokenId, BoundedString, BoundedMetadata> {
	pub founder: AccountId,
	pub account_id: AccountId,
	pub token_id: Option<TokenId>,
	pub token_address: Option<BoundedString>,
	pub status: DaoStatus,
	pub config: DaoConfig<BoundedString, BoundedMetadata>,
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

#[derive(
	Encode, Decode, Copy, Clone, Default, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen,
)]
pub enum AccountTokenBalance {
	#[default]
	// Insufficient
	Insufficient,
	Sufficient,
	// An off-chain action should be performed
	Offchain,
}

pub trait DaoProvider<Hash> {
	type Id;
	type AccountId;
	type AssetId;
	type Policy;

	fn exists(id: Self::Id) -> Result<(), DispatchError>;
	fn dao_account_id(id: Self::Id) -> Self::AccountId;
	fn policy(id: Self::Id) -> Result<Self::Policy, DispatchError>;
	fn count() -> u32;
	fn ensure_member(id: Self::Id, who: &Self::AccountId) -> Result<bool, DispatchError>;
	fn ensure_proposal_allowed(
		id: Self::Id,
		who: &Self::AccountId,
		threshold: u32,
		hash: Hash,
		length_bound: u32,
	) -> Result<AccountTokenBalance, DispatchError>;
	fn ensure_token_balance(
		id: Self::Id,
		who: &Self::AccountId,
	) -> Result<AccountTokenBalance, DispatchError>;
}

pub trait InitializeDaoMembers<DaoId, AccountId> {
	fn initialize_members(dao_id: DaoId, members: Vec<AccountId>) -> Result<(), DispatchError>;
}

pub trait ContainsDaoMember<DaoId, AccountId> {
	fn contains(dao_id: DaoId, who: &AccountId) -> Result<bool, DispatchError>;
}

pub trait ApprovePropose<DaoId, AccountId, Hash> {
	fn approve_propose(
		dao_id: DaoId,
		who: AccountId,
		threshold: u32,
		hash: Hash,
		length_bound: u32,
	) -> Result<(), DispatchError>;
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
