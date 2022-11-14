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
pub struct DaoPolicy<AccountId> {
	/// Fraction of a proposal's value that should be bonded in order to place the proposal.
	/// An accepted proposal gets these back. A rejected proposal does not.
	pub proposal_bond: u32, //TODO: static value or percentage???
	/// Minimum amount of funds that should be placed in a deposit for making a proposal.
	pub proposal_bond_min: u128,
	/// Maximum amount of funds that should be placed in a deposit for making a proposal.
	pub proposal_bond_max: Option<u128>,
	/// In millis
	pub proposal_period: u32,
	//TODO: ??
	pub prime_account: AccountId,
	// TODO: use max members for account length
	pub approve_origin: (u32, u32),
	pub reject_origin: (u32, u32),
}

#[derive(Encode, Decode, Default, Clone, PartialEq, Eq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
pub struct Dao<AccountId, TokenId, BoundedString, BoundedMetadata> {
	pub founder: AccountId,
	pub account_id: AccountId,
	pub token_id: TokenId,
	pub config: DaoConfig<BoundedString, BoundedMetadata>,
}

pub trait DaoProvider {
	type Id;
	type AccountId;
	type Policy;

	fn exists(id: Self::Id) -> Result<(), DispatchError>;
	fn dao_account_id(id: Self::Id) -> Self::AccountId;
	fn policy(id: Self::Id) -> Result<Self::Policy, DispatchError>;
	fn count() -> u32;
}

// TODO: rename to InitializeMembers?
pub trait InitializeDaoMembers<DaoId, AccountId> {
	fn initialize_members(dao_id: DaoId, members: Vec<AccountId>) -> Result<(), DispatchError>;
}

/// Trait for type that can handle incremental changes to a set of account IDs.
pub trait ChangeDaoMembers<DaoId, AccountId: Clone + Ord> {
	/// A number of members `incoming` just joined the set and replaced some `outgoing` ones. The
	/// new set is given by `new`, and need not be sorted.
	///
	/// This resets any previous value of prime.
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
	/// NOTE: This is the only function that needs to be implemented in `ChangeMembers`.
	///
	/// This resets any previous value of prime.
	fn change_members_sorted(
		dao_id: DaoId,
		incoming: &[AccountId],
		outgoing: &[AccountId],
		sorted_new: &[AccountId],
	);

	/// Set the new members; they **must already be sorted**. This will compute the diff and use it
	/// to call `change_members_sorted`.
	///
	/// This resets any previous value of prime.
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

	/// Set the prime member.
	fn set_prime(_dao_id: DaoId, _prime: Option<AccountId>) {}

	/// Get the current prime.
	fn get_prime(_dao_id: DaoId) -> Option<AccountId> {
		None
	}
}

impl<D, T: Clone + Ord> ChangeDaoMembers<D, T> for () {
	fn change_members(_: D, _: &[T], _: &[T], _: Vec<T>) {}
	fn change_members_sorted(_: D, _: &[T], _: &[T], _: &[T]) {}
	fn set_members_sorted(_: D, _: &[T], _: &[T]) {}
	fn set_prime(_: D, _: Option<T>) {}
}

pub fn de_string_to_bytes<'de, D>(de: D) -> Result<Vec<u8>, D::Error>
where
	D: Deserializer<'de>,
{
	let s: &str = Deserialize::deserialize(de)?;
	Ok(s.as_bytes().to_vec())
}

pub fn de_string_to_u128<'de, D>(de: D) -> Result<u128, D::Error>
where
	D: Deserializer<'de>,
{
	let s: &str = Deserialize::deserialize(de)?;
	Ok(s.parse::<u128>().unwrap())
}
