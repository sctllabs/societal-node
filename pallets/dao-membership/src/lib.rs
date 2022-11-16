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
//! Allows control of DAO membership of a set of `AccountId`s, useful for managing membership of of a
//! collective.

// Ensure we're `no_std` when compiling for Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	dispatch::DispatchError,
	traits::{EnsureOriginWithArg, Get},
	BoundedVec,
};
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

use dao_primitives::{ChangeDaoMembers, DaoPolicy, DaoProvider, InitializeDaoMembers};

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
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self, I>> + IsType<<Self as frame_system::Config>::Event>;

		/// Required origin for adding a member (though can always be Root).
		type ApproveOrigin: EnsureOriginWithArg<Self::Origin, (u32, u32)>;

		/// The receiver of the signal for when the membership has been initialized. This happens
		/// pre-genesis and will usually be the same as `MembershipChanged`. If you need to do
		/// something different on initialization, then you can change this accordingly.
		type MembershipInitialized: InitializeDaoMembers<DaoId, Self::AccountId>;

		/// The receiver of the signal for when the membership has changed.
		type MembershipChanged: ChangeDaoMembers<DaoId, Self::AccountId>;

		/// The maximum number of members that this membership can have.
		///
		/// This is used for benchmarking. Re-run the benchmarks if this changes.
		///
		/// This is enforced in the code; the membership size can not exceed this limit.
		type MaxMembers: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;

		// TODO: rework providers
		type DaoProvider: DaoProvider<Id = u32, AccountId = Self::AccountId, Policy = DaoPolicy>;
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
		MemberAdded,
		/// The given member was removed; see the transaction for who.
		MemberRemoved,
		/// Two members were swapped; see the transaction for who.
		MembersSwapped,
		/// The membership was reset; see the transaction for who the new set is.
		MembersReset,
		/// One of the members' keys changed.
		KeyChanged,
		/// Phantom member, never used.
		Dummy { _phantom_data: PhantomData<(T::AccountId, <T as Config<I>>::Event)> },
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
		///
		/// May only be called from `T::AddOrigin`.
		#[pallet::weight(50_000_000)]
		pub fn add_member(
			origin: OriginFor<T>,
			dao_id: DaoId,
			who: T::AccountId,
		) -> DispatchResult {
			T::ApproveOrigin::ensure_origin(
				origin,
				&T::DaoProvider::policy(dao_id)?.approve_origin,
			)?;

			let mut members = <Members<T, I>>::get(dao_id);
			let location = members.binary_search(&who).err().ok_or(Error::<T, I>::AlreadyMember)?;
			members
				.try_insert(location, who.clone())
				.map_err(|_| Error::<T, I>::TooManyMembers)?;

			<Members<T, I>>::insert(dao_id, &members);

			T::MembershipChanged::change_members_sorted(dao_id, &[who], &[], &members[..]);

			Self::deposit_event(Event::MemberAdded);
			Ok(())
		}

		/// Remove a member `who` from the set.
		///
		/// May only be called from `T::RemoveOrigin`.
		#[pallet::weight(50_000_000)]
		pub fn remove_member(
			origin: OriginFor<T>,
			dao_id: DaoId,
			who: T::AccountId,
		) -> DispatchResult {
			T::ApproveOrigin::ensure_origin(
				origin,
				&T::DaoProvider::policy(dao_id)?.approve_origin,
			)?;

			let mut members = <Members<T, I>>::get(dao_id);
			let location = members.binary_search(&who).ok().ok_or(Error::<T, I>::NotMember)?;
			members.remove(location);

			<Members<T, I>>::insert(dao_id, &members);

			T::MembershipChanged::change_members_sorted(dao_id, &[], &[who], &members[..]);

			Self::deposit_event(Event::MemberRemoved);
			Ok(())
		}

		/// Swap out one member `remove` for another `add`.
		///
		/// May only be called from `T::SwapOrigin`.
		#[pallet::weight(50_000_000)]
		pub fn swap_member(
			origin: OriginFor<T>,
			dao_id: DaoId,
			remove: T::AccountId,
			add: T::AccountId,
		) -> DispatchResult {
			T::ApproveOrigin::ensure_origin(
				origin,
				&T::DaoProvider::policy(dao_id)?.approve_origin,
			)?;

			if remove == add {
				return Ok(())
			}

			let mut members = <Members<T, I>>::get(dao_id);
			let location = members.binary_search(&remove).ok().ok_or(Error::<T, I>::NotMember)?;
			let _ = members.binary_search(&add).err().ok_or(Error::<T, I>::AlreadyMember)?;
			members[location] = add.clone();
			members.sort();

			<Members<T, I>>::insert(dao_id, &members);

			T::MembershipChanged::change_members_sorted(dao_id, &[add], &[remove], &members[..]);

			Self::deposit_event(Event::MembersSwapped);
			Ok(())
		}

		/// Change the membership to a new set, disregarding the existing membership. Be nice and
		/// pass `members` pre-sorted.
		///
		/// May only be called from `T::ResetOrigin`.
		#[pallet::weight(50_000_000)]
		pub fn reset_members(
			origin: OriginFor<T>,
			dao_id: DaoId,
			members: Vec<T::AccountId>,
		) -> DispatchResult {
			T::ApproveOrigin::ensure_origin(
				origin,
				&T::DaoProvider::policy(dao_id)?.approve_origin,
			)?;

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
		///
		/// May only be called from `Signed` origin of a current member.
		#[pallet::weight(50_000_000)]
		pub fn change_key(
			origin: OriginFor<T>,
			dao_id: DaoId,
			new: T::AccountId,
		) -> DispatchResult {
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
					&[remove.clone()],
					&members[..],
				);
			}

			Self::deposit_event(Event::KeyChanged);
			Ok(())
		}
	}
}

// TODO
// impl<T: Config<I>, I: 'static> Contains<T::AccountId> for Pallet<T, I> {
// 	fn contains(dao_id: DaoId, t: &T::AccountId) -> bool {
// 		Self::members(dao_id).binary_search(t).is_ok()
// 	}
// }

/// A trait for a set which can enumerate its members in order.
pub trait DaoSortedMembers<T: Ord> {
	/// Get a vector of all members in the set, ordered.
	fn sorted_members(dao_id: DaoId) -> Vec<T>;

	/// Return `true` if this "contains" the given value `t`.
	fn contains(dao_id: DaoId, t: &T) -> bool {
		Self::sorted_members(dao_id).binary_search(t).is_ok()
	}

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

// TODO: make abstraction to frame InitializeMembers
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
