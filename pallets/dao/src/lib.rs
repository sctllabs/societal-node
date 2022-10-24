#![cfg_attr(not(feature = "std"), no_std)]

use codec::MaxEncodedLen;
use frame_support::{
	codec::{Decode, Encode},
	traits::{Currency, Get, ReservableCurrency, UnfilteredDispatchable},
	weights::GetDispatchInfo,
	BoundedVec, PalletId,
};
/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
pub use pallet::*;
use scale_info::TypeInfo;
use sp_runtime::{traits::AccountIdConversion, RuntimeDebug};
use sp_std::prelude::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod pallet {
	pub use super::*;
	use frame_support::{pallet_prelude::*, traits::UnixTime};
	use frame_system::pallet_prelude::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(4);
	const MAX_DESCRIPTION_LENGTH: u32 = 300;

	type DescriptionLength = ConstU32<MAX_DESCRIPTION_LENGTH>;

	#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
	pub struct OrgConfig {
		/// Name of the DAO.
		pub name: BoundedVec<u8, ConstU32<15>>,
		/// Purpose of this DAO.
		pub purpose: BoundedVec<u8, ConstU32<150>>,
		//TODO: Generic metadata.
	}

	#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
	pub struct Organization<AccountId> {
		pub founder: AccountId,
		// Treasury
		pub account_id: AccountId,
		pub org_type: u32,
		pub description: BoundedVec<u8, ConstU32<MAX_DESCRIPTION_LENGTH>>,
		pub next_proposal_id: u32,
		pub config: OrgConfig,
	}

	#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
	pub enum ProposalStatus {
		#[default]
		InProgress,
		/// If quorum voted yes, this proposal is successfully approved.
		Approved,
		/// If quorum voted no, this proposal is rejected. Bond is returned.
		Rejected,
		/// If quorum voted to remove (e.g. spam), this proposal is rejected and bond is not
		/// returned. Interfaces shouldn't show removed proposals.
		Removed,
		/// Expired after period of time.
		Expired,
		/// If proposal was moved to Hub or somewhere else.
		Moved,
		/// If proposal has failed when finalizing. Allowed to re-finalize again to either expire
		/// or approved.
		Failed,
	}

	#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
	pub struct Proposal<AccountId> {
		/// Original proposer.
		pub proposer: AccountId,
		/// Description of this proposal.
		// pub description: String,
		/// Kind of proposal with relevant information.
		// pub kind: ProposalKind,
		/// Current status of the proposal.
		// pub status: ProposalStatus,
		/// Count of votes per role per decision: yes / no / spam.
		// pub vote_counts: HashMap<String, [Balance; 3]>,
		/// Map of who voted and how.
		// pub votes: HashMap<AccountId, Vote>,
		/// Submission time (for voting period).
		pub submission_time: u128,
		pub status: ProposalStatus,
	}

	type OrganizationOf<T> = Organization<<T as frame_system::Config>::AccountId>;
	type ProposalOf<T> = Proposal<<T as frame_system::Config>::AccountId>;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	// #[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type Call: Parameter + UnfilteredDispatchable<Origin = Self::Origin> + GetDispatchInfo;
		type Currency: ReservableCurrency<Self::AccountId>;
		#[pallet::constant]
		type PalletId: Get<PalletId>;
		type TaxInPercent: Get<u32>;
		type SupervisorOrigin: EnsureOrigin<Self::Origin>;
		// type MaxMembers: Get<u32>;
		type TimeProvider: UnixTime;
		// type DescLimit: Get<u32>;
	}

	// The pallet's runtime storage items.
	// https://docs.substrate.io/main-docs/build/runtime-storage/
	#[pallet::storage]
	#[pallet::getter(fn next_org_id)]
	// Learn more about declaring storage items:
	// https://docs.substrate.io/main-docs/build/runtime-storage/#declaring-storage-items
	pub(super) type NextOrgId<T> = StorageValue<_, u32, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn organizations)]
	pub(super) type Organizations<T: Config> =
		StorageMap<_, Blake2_128Concat, u32, OrganizationOf<T>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn proposals)]
	pub(super) type Proposals<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		u32,
		Blake2_128Concat,
		u32,
		ProposalOf<T>,
		OptionQuery,
	>;

	// #[pallet::storage]
	// #[pallet::getter(fn organizations)]
	// pub(super) type Organizations<T: Config> =
	// 	StorageDoubleMap<_, Blake2_128Concat, u32, OrganizationOf<T>, OptionQuery>;

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/main-docs/build/events-errors/
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event documentation should end with an array that provides descriptive names for event
		/// parameters. [something, who]
		OrgRegistered(u32, T::AccountId),
		OrgJoined(u32, T::AccountId),
		ProposalAdded(u32, T::AccountId),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		/// Error names should be descriptive.
		NoneValue,
		/// Errors should have helpful documentation associated with them.
		OrganizationNotExist,
		DescriptionTooLong,
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// An example dispatchable that takes a singles value as a parameter, writes the value to
		/// storage and emits an event. This function must be dispatched by a signed extrinsic.
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn create_organization(
			origin: OriginFor<T>,
			name: Vec<u8>,
			description: Vec<u8>,
			purpose: Vec<u8>,
		) -> DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			// This function will return an error if the extrinsic is not signed.
			// https://docs.substrate.io/main-docs/build/origins/
			let who = ensure_signed(origin)?;

			let mut org_description = Default::default();
			match BoundedVec::<u8, ConstU32<MAX_DESCRIPTION_LENGTH>>::try_from(description.clone())
			{
				Ok(description) => {
					org_description = description;
				},
				Err(_) => return Err(Error::<T>::OrganizationNotExist.into()),
			}

			// TODO: check name length
			// TODO: check purpose length

			let org_id = <NextOrgId<T>>::get();
			<NextOrgId<T>>::put(org_id.checked_add(1).unwrap());

			let org = Organization {
				founder: who.clone(),
				account_id: Self::account_id(org_id),
				org_type: 1,
				description: org_description,
				next_proposal_id: 0,
				config: OrgConfig {
					name: BoundedVec::<u8, ConstU32<15>>::try_from(name.clone()).ok().unwrap(),
					purpose: BoundedVec::<u8, ConstU32<150>>::try_from(purpose.clone())
						.ok()
						.unwrap(),
				},
			};

			Organizations::<T>::insert(org_id, org);

			// Emit an event.
			Self::deposit_event(Event::OrgRegistered(org_id, who));

			// Return a successful DispatchResultWithPostInfo
			Ok(().into())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn add_proposal(origin: OriginFor<T>, org_id: u32) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let org = Organizations::<T>::get(org_id);
			match org {
				Some(mut org) => {
					let proposal_id = org.next_proposal_id;

					let proposal = Proposal {
						proposer: who.clone(),
						submission_time: T::TimeProvider::now().as_millis(),
						status: ProposalStatus::InProgress,
					};

					Proposals::<T>::insert(org_id, proposal_id, proposal);

					org.next_proposal_id = org.next_proposal_id.checked_add(1).unwrap();
					Organizations::<T>::insert(org_id, org);
				},
				None => return Err(Error::<T>::OrganizationNotExist.into()),
			}

			Self::deposit_event(Event::ProposalAdded(org_id, who));

			Ok(().into())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn act_proposal(
			origin: OriginFor<T>,
			org_id: u32,
			proposal_id: u32,
			vote: bool,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let org = Organizations::<T>::get(org_id);

			// TODO: add asserts

			let mut proposal = Proposals::<T>::get(org_id, proposal_id).unwrap();

			if vote {
				proposal.status = ProposalStatus::Approved;
			} else {
				proposal.status = ProposalStatus::Rejected;
			}

			Proposals::<T>::insert(org_id, proposal_id, proposal);

			Ok(().into())
		}

		// An example dispatchable that may throw a custom error.
		// #[pallet::weight(10_000 + T::DbWeight::get().reads_writes(1,1))]
		// pub fn cause_error(origin: OriginFor<T>) -> DispatchResult {
		// 	let _who = ensure_signed(origin)?;

		// 	// Read a value from storage.
		// 	match <Something<T>>::get() {
		// 		// Return an error if the value has not been set.
		// 		None => return Err(Error::<T>::NoneValue.into()),
		// 		Some(old) => {
		// 			// Increment the value read from storage; will error in the event of overflow.
		// 			let new = old.checked_add(1).ok_or(Error::<T>::StorageOverflow)?;
		// 			// Update the value in storage with the incremented result.
		// 			<Something<T>>::put(new);
		// 			Ok(())
		// 		},
		// 	}
		// }
	}

	impl<T: Config> Pallet<T> {
		fn account_id(app_id: u32) -> T::AccountId {
			if app_id == 0 {
				T::PalletId::get().into_account_truncating()
			} else {
				T::PalletId::get().into_sub_account_truncating(app_id)
			}
		}

		fn u128_to_balance(cost: u128) -> BalanceOf<T> {
			TryInto::<BalanceOf<T>>::try_into(cost).ok().unwrap()
		}

		fn balance_to_u128(balance: BalanceOf<T>) -> u128 {
			TryInto::<u128>::try_into(balance).ok().unwrap()
		}

		/// refer https://github.com/paritytech/substrate/blob/743accbe3256de2fc615adcaa3ab03ebdbbb4dbd/frame/treasury/src/lib.rs#L351
		///
		/// This actually does computation. If you need to keep using it, then make sure you cache
		/// the value and only call this once.
		pub fn validate_member(account_id: T::AccountId, ord_id: u32) -> bool {
			if !Organizations::<T>::contains_key(ord_id) {
				false
			} else {
				true
				// let members = Organizations::<T>::get(ord_id).unwrap().members;
				// match members.binary_search(&account_id) {
				// 	Ok(_) => true,
				// 	Err(_) => false,
				// }
			}
		}
	}
}
