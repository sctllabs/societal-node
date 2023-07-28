// This file is part of Substrate.

// Copyright (C) 2019-2022 Parity Technologies (UK) Ltd.
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

// Re-purposing the pallet to manage membership for the DAOs created by the pallet-dao factory:
// - re-worked pallet storage to persist membership data for each DAO
// - updated pallet extrinsic functions adding dao support
// - added support for DaoProvider retrieving custom configuration for each DAO
// - updated origins using EnsureOriginWithArg to support custom configuration by DaoProvider
// - removed GenesisConfig
// - removed support for 'prime' member
// - exported benchmarking/tests to separate modules

//! # DAO Membership Module
//!
//! Allows control of DAO membership of a set of `AccountId`s, useful for managing membership of of
//! a collective.

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	dispatch::DispatchError, pallet_prelude::DispatchResult, traits::Get, BoundedVec,
};
use frame_system::pallet_prelude::OriginFor;
use sp_std::prelude::*;

pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub use pallet::*;
pub use weights::WeightInfo;

use dao_primitives::{
	ChangeDaoMembers, ContainsDaoMember, DaoPolicy, DaoProvider, InitializeDaoMembers,
	RemoveDaoMembers,
};

/// Dao ID. Just a `u32`.
pub type DaoId = u32;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(4);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self, I>>
			+ IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The receiver of the signal for when the membership has been initialized. This happens
		/// pre-genesis and will usually be the same as `MembershipChanged`. If you need to do
		/// something different on initialization, then you can change this accordingly.
		type MembershipInitialized: InitializeDaoMembers<DaoId, Self::AccountId>;

		/// The receiver of the signal for when the membership has changed.
		type MembershipChanged: ChangeDaoMembers<DaoId, Self::AccountId>;

		/// The receiver of the signal for when the membership is removed.
		type MembershipRemoved: RemoveDaoMembers<DaoId>;

		/// The maximum number of members that this membership can have.
		///
		/// This is used for benchmarking. Re-run the benchmarks if this changes.
		///
		/// This is enforced in the code; the membership size can not exceed this limit.
		type MaxMembers: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;

		type DaoProvider: DaoProvider<
			Self::AccountId,
			<Self as frame_system::Config>::Hash,
			Id = u32,
			Policy = DaoPolicy,
			Origin = OriginFor<Self>,
		>;
	}

	/// The current membership, stored as an ordered Vec.
	#[pallet::storage]
	#[pallet::getter(fn members)]
	pub type Members<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Twox64Concat, DaoId, BoundedVec<T::AccountId, T::MaxMembers>, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		/// The given member was added; see the transaction for who.
		MemberAdded { dao_id: DaoId, member: T::AccountId },
		/// The given member was removed; see the transaction for who.
		MemberRemoved { dao_id: DaoId, member: T::AccountId },
		/// Two members were swapped; see the transaction for who.
		MembersSwapped { dao_id: DaoId, remove: T::AccountId, add: T::AccountId },
		/// The membership was reset; see the transaction for who the new set is.
		MembersReset,
		/// One of the members' keys changed.
		KeyChanged,
		/// Phantom member, never used.
		Dummy { _phantom_data: PhantomData<(T::AccountId, <T as Config<I>>::RuntimeEvent)> },
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		/// Already a member.
		AlreadyMember,
		/// Not a member.
		NotMember,
		/// Too many members.
		TooManyMembers,
	}

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		/// Add a member `who` to the set.
		#[pallet::weight(T::WeightInfo::add_member())]
		#[pallet::call_index(0)]
		pub fn add_member(
			origin: OriginFor<T>,
			dao_id: DaoId,
			who: T::AccountId,
		) -> DispatchResult {
			T::DaoProvider::ensure_approved(origin, dao_id)?;

			let mut members = <Members<T, I>>::get(dao_id);
			let location = members.binary_search(&who).err().ok_or(Error::<T, I>::AlreadyMember)?;
			members
				.try_insert(location, who.clone())
				.map_err(|_| Error::<T, I>::TooManyMembers)?;

			<Members<T, I>>::insert(dao_id, &members);

			T::MembershipChanged::change_members_sorted(dao_id, &[who.clone()], &[], &members[..]);

			Self::deposit_event(Event::MemberAdded { dao_id, member: who });
			Ok(())
		}

		/// Remove a member `who` from the set.
		#[pallet::weight(T::WeightInfo::remove_member())]
		#[pallet::call_index(1)]
		pub fn remove_member(
			origin: OriginFor<T>,
			dao_id: DaoId,
			who: T::AccountId,
		) -> DispatchResult {
			T::DaoProvider::ensure_approved(origin, dao_id)?;

			let mut members = <Members<T, I>>::get(dao_id);
			let location = members.binary_search(&who).ok().ok_or(Error::<T, I>::NotMember)?;
			members.remove(location);

			<Members<T, I>>::insert(dao_id, &members);

			T::MembershipChanged::change_members_sorted(dao_id, &[], &[who.clone()], &members[..]);

			Self::deposit_event(Event::MemberRemoved { dao_id, member: who });
			Ok(())
		}

		/// Swap out one member `remove` for another `add`.
		#[pallet::weight(T::WeightInfo::swap_member())]
		#[pallet::call_index(2)]
		pub fn swap_member(
			origin: OriginFor<T>,
			dao_id: DaoId,
			remove: T::AccountId,
			add: T::AccountId,
		) -> DispatchResult {
			T::DaoProvider::ensure_approved(origin, dao_id)?;

			if remove == add {
				return Ok(())
			}

			let mut members = <Members<T, I>>::get(dao_id);
			let location = members.binary_search(&remove).ok().ok_or(Error::<T, I>::NotMember)?;
			let _ = members.binary_search(&add).err().ok_or(Error::<T, I>::AlreadyMember)?;
			members[location] = add.clone();
			members.sort();

			<Members<T, I>>::insert(dao_id, &members);

			T::MembershipChanged::change_members_sorted(
				dao_id,
				&[add.clone()],
				&[remove.clone()],
				&members[..],
			);

			Self::deposit_event(Event::MembersSwapped { dao_id, remove, add });
			Ok(())
		}

		/// Change the membership to a new set, disregarding the existing membership. Be nice and
		/// pass `members` pre-sorted.
		#[pallet::weight(T::WeightInfo::reset_members())]
		#[pallet::call_index(3)]
		pub fn reset_members(
			origin: OriginFor<T>,
			dao_id: DaoId,
			members: Vec<T::AccountId>,
		) -> DispatchResult {
			T::DaoProvider::ensure_approved(origin, dao_id)?;

			let mut members: BoundedVec<T::AccountId, T::MaxMembers> =
				BoundedVec::try_from(members).map_err(|_| Error::<T, I>::TooManyMembers)?;
			members.sort();
			<Members<T, I>>::mutate(dao_id, |m| {
				T::MembershipChanged::set_members_sorted(dao_id, &members[..], m);
				*m = members;
			});

			Self::deposit_event(Event::MembersReset);
			Ok(())
		}

		/// Swap out the sending member for some other key `new`.
		#[pallet::weight((T::WeightInfo::change_key(), DispatchClass::Normal, Pays::No))]
		#[pallet::call_index(4)]
		pub fn change_key(
			origin: OriginFor<T>,
			dao_id: DaoId,
			new: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let remove = ensure_signed(origin)?;

			if remove != new {
				let mut members = <Members<T, I>>::get(dao_id);
				let location =
					members.binary_search(&remove).ok().ok_or(Error::<T, I>::NotMember)?;
				let _ = members.binary_search(&new).err().ok_or(Error::<T, I>::AlreadyMember)?;
				members[location] = new.clone();
				members.sort();

				<Members<T, I>>::insert(dao_id, &members);

				T::MembershipChanged::change_members_sorted(
					dao_id,
					&[new.clone()],
					&[remove],
					&members[..],
				);
			}

			Self::deposit_event(Event::KeyChanged);

			Ok((Some(T::WeightInfo::change_key()), Pays::No).into())
		}
	}
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
	/// Check whether `who` is a member of the collective.
	pub fn is_member(dao_id: DaoId, who: &T::AccountId) -> bool {
		Self::sorted_members(dao_id).binary_search(who).is_ok()
	}
}

impl<T: Config<I>, I: 'static> ContainsDaoMember<DaoId, T::AccountId> for Pallet<T, I> {
	fn contains(dao_id: DaoId, who: &T::AccountId) -> Result<bool, DispatchError> {
		if Self::is_member(dao_id, who) {
			return Ok(true)
		}

		Err(Error::<T, I>::NotMember.into())
	}
}

/// A trait for a set which can enumerate its members in order.
pub trait DaoSortedMembers<T: Ord> {
	/// Get a vector of all members in the set, ordered.
	fn sorted_members(dao_id: DaoId) -> Vec<T>;

	/// Get the number of items in the set.
	fn count(dao_id: DaoId) -> usize {
		Self::sorted_members(dao_id).len()
	}

	/// Add an item that would satisfy `contains`. It does not make sure any other
	/// state is correctly maintained or generated.
	///
	/// **Should be used for benchmarking only!!!**
	#[cfg(feature = "runtime-benchmarks")]
	fn add(_t: &T) {
		unimplemented!()
	}
}
impl<T: Config<I>, I: 'static> DaoSortedMembers<T::AccountId> for Pallet<T, I> {
	fn sorted_members(dao_id: DaoId) -> Vec<T::AccountId> {
		Self::members(dao_id).to_vec()
	}

	fn count(dao_id: DaoId) -> usize {
		Members::<T, I>::decode_len(dao_id).unwrap_or(0)
	}
}

impl<T: Config<I>, I: 'static> InitializeDaoMembers<DaoId, T::AccountId> for Pallet<T, I> {
	fn initialize_members(
		dao_id: DaoId,
		source_members: Vec<T::AccountId>,
	) -> Result<(), DispatchError> {
		if !source_members.is_empty() {
			assert!(<Members<T, I>>::get(dao_id).is_empty(), "Members are already initialized!");

			let mut members: BoundedVec<T::AccountId, T::MaxMembers> =
				BoundedVec::try_from(source_members).map_err(|_| Error::<T, I>::TooManyMembers)?;
			members.sort();
			T::MembershipInitialized::initialize_members(dao_id, members.clone().into())?;
			<Members<T, I>>::insert(dao_id, members);
		}

		Ok(())
	}
}

impl<T: Config<I>, I: 'static> RemoveDaoMembers<DaoId> for Pallet<T, I> {
	fn remove_members(dao_id: DaoId, purge: bool) -> Result<(), DispatchError> {
		<Members<T, I>>::remove(dao_id);

		T::MembershipRemoved::remove_members(dao_id, purge)?;

		Ok(())
	}
}
