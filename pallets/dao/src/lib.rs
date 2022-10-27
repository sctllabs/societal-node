#![cfg_attr(not(feature = "std"), no_std)]

use codec::MaxEncodedLen;
use frame_support::{
	codec::{Decode, Encode},
	dispatch::DispatchResult,
	traits::{
		tokens::fungibles::{Inspect, Mutate},
		Currency,
		ExistenceRequirement::KeepAlive,
		Get, ReservableCurrency, UnfilteredDispatchable, UnixTime,
	},
	weights::GetDispatchInfo,
	BoundedVec, PalletId,
};
pub use pallet::*;
use scale_info::TypeInfo;
use serde::{self, Deserialize, Serialize};
use sp_runtime::{traits::AccountIdConversion, RuntimeDebug};
use sp_std::prelude::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

// TODO
type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

type OrganizationOf<T> = Organization<
	<T as frame_system::Config>::AccountId,
	<T as pallet_assets::Config>::AssetId,
	BoundedVec<u8, <T as Config>::DaoStringLimit>,
	BoundedVec<u8, <T as Config>::DaoStringLimit>,
>;
type ProposalOf<T> = Proposal<<T as frame_system::Config>::AccountId>;

type AssetId<T> = <T as Config>::AssetId;
type Balance<T> = <T as Config>::Balance;

#[frame_support::pallet]
pub mod pallet {
	pub use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use serde::Deserializer;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(4);

	#[derive(
		Encode, Decode, Default, Clone, PartialEq, TypeInfo, RuntimeDebug, Serialize, Deserialize,
	)]
	pub struct DaoTokenMetadata {
		#[serde(deserialize_with = "de_string_to_bytes")]
		name: Vec<u8>,
		#[serde(deserialize_with = "de_string_to_bytes")]
		symbol: Vec<u8>,
		decimals: u8,
	}

	#[derive(
		Encode, Decode, Default, Clone, PartialEq, TypeInfo, RuntimeDebug, Serialize, Deserialize,
	)]
	pub struct DaoGovernanceToken {
		token_id: u32,
		metadata: DaoTokenMetadata,
		#[serde(deserialize_with = "de_string_to_u128")]
		min_balance: u128,
	}

	#[derive(
		Encode, Decode, Default, Clone, PartialEq, TypeInfo, RuntimeDebug, Serialize, Deserialize,
	)]
	pub struct DaoPolicy {
		pub proposal_bond: u128,
		pub proposal_period: u64,
	}

	#[derive(
		Encode, Decode, Default, Clone, PartialEq, TypeInfo, RuntimeDebug, Serialize, Deserialize,
	)]
	pub struct OrganizationPayload {
		#[serde(deserialize_with = "de_string_to_bytes")]
		name: Vec<u8>,
		#[serde(deserialize_with = "de_string_to_bytes")]
		purpose: Vec<u8>,
		#[serde(deserialize_with = "de_string_to_bytes")]
		metadata: Vec<u8>,
		token: Option<DaoGovernanceToken>,
		token_id: Option<u32>,
		policy: DaoPolicy,
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

	#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
	pub struct OrganizationConfig<BoundedString, BoundedMetadata> {
		/// Name of the DAO.
		pub name: BoundedString,
		/// Purpose of this DAO.
		pub purpose: BoundedString,
		/// Generic metadata. Can be used to store additional data.
		pub metadata: BoundedMetadata,
	}

	#[derive(Encode, Decode, Default, Clone, PartialEq, TypeInfo, RuntimeDebug, MaxEncodedLen)]
	pub struct Organization<AccountId, TokenId, BoundedString, BoundedMetadata> {
		pub founder: AccountId,
		pub account_id: AccountId,
		pub token_id: TokenId,
		pub next_proposal_id: u32,
		pub config: OrganizationConfig<BoundedString, BoundedMetadata>,
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

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_assets::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type Call: Parameter + UnfilteredDispatchable<Origin = Self::Origin> + GetDispatchInfo;
		type Currency: ReservableCurrency<Self::AccountId>;
		type SupervisorOrigin: EnsureOrigin<Self::Origin>;
		type TimeProvider: UnixTime;

		type AssetId: IsType<<Self as pallet_assets::Config>::AssetId>
			+ Parameter
			+ From<u32>
			+ Ord
			+ Copy;

		type Balance: IsType<<Self as pallet_assets::Config>::Balance>
			+ Parameter
			+ From<u128>
			+ Ord
			+ Copy;

		#[pallet::constant]
		type PalletId: Get<PalletId>;

		#[pallet::constant]
		type DaoStringLimit: Get<u32>;

		#[pallet::constant]
		type DaoMetadataLimit: Get<u32>;
	}

	#[pallet::storage]
	#[pallet::getter(fn next_org_id)]
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

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event documentation should end with an array that provides descriptive names for event
		/// parameters. [something, who]
		OrgRegistered(u32, T::AccountId),
		OrgJoined(u32, T::AccountId),
		ProposalAdded(u32, T::AccountId),
	}

	#[pallet::error]
	pub enum Error<T> {
		NoneValue,
		OrganizationNotExist,
		NameTooLong,
		PurposeTooLong,
		MetadataTooLong,
		TokenNotProvided,
		TokenAlreadyExists,
		TokenNotExists,
		TokenCreateFailed,
		InvalidInput,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		// TODO: calculate dynamic weight
		#[pallet::weight(10_000 + T::DbWeight::get().writes(4))]
		pub fn create_organization(origin: OriginFor<T>, data: Vec<u8>) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			let payload = serde_json::from_slice::<OrganizationPayload>(&data);
			if payload.is_err() {
				log::info!("err: {:?}", payload);

				return Err(Error::<T>::InvalidInput.into())
			}

			let organization = payload.unwrap();

			let org_id = <NextOrgId<T>>::get();
			let org_account_id = Self::account_id(org_id);

			let mut org_name = Default::default();
			match BoundedVec::<u8, T::DaoStringLimit>::try_from(organization.name.clone()) {
				Ok(name) => {
					org_name = name;
				},
				Err(_) => return Err(Error::<T>::OrganizationNotExist.into()),
			}

			let mut org_purpose = Default::default();
			match BoundedVec::<u8, T::DaoStringLimit>::try_from(organization.purpose.clone()) {
				Ok(purpose) => {
					org_purpose = purpose;
				},
				Err(_) => return Err(Error::<T>::OrganizationNotExist.into()),
			}

			let mut org_metadata = Default::default();
			match BoundedVec::<u8, T::DaoStringLimit>::try_from(organization.metadata.clone()) {
				Ok(metadata) => {
					org_metadata = metadata;
				},
				Err(_) => return Err(Error::<T>::OrganizationNotExist.into()),
			}

			// TODO: revise this - need some minimal balance to mint tokens to
			let _transfer_res = <T as Config>::Currency::transfer(
				&who,
				&org_account_id,
				Self::u128_to_balance_of(100000000),
				KeepAlive,
			);

			let mut has_token_id: Option<AssetId<T>> = None;

			if let Some(token) = organization.token {
				let token_id = Self::u32_to_asset_id(token.token_id);
				has_token_id = Some(token_id);

				let metadata = token.metadata;
				let min_balance = Self::u128_to_balance(token.min_balance).into();

				// considering organization as the governance token admin
				let token_admin = <T::Lookup as sp_runtime::traits::StaticLookup>::unlookup(
					org_account_id.clone(),
				);

				let issuance = pallet_assets::Pallet::<T>::total_issuance(token_id.into())
					.try_into()
					.unwrap_or(0u128);

				if issuance > 0 {
					return Err(Error::<T>::TokenAlreadyExists.into())
				} else {
					log::info!("issuing token: {:?}", token_id);
					let create_result = pallet_assets::Pallet::<T>::create(
						origin.clone(),
						token_id.into(),
						token_admin,
						min_balance,
					);
					match create_result {
						Ok(_) => {},
						Err(e) => {
							log::info!("error occurred while issuing token: {:?}", e);

							return Err(Error::<T>::TokenCreateFailed.into())
						},
					}

					log::info!("setting metadata for token: {:?}", token_id);
					let metadata_result = pallet_assets::Pallet::<T>::set_metadata(
						origin.clone(),
						token_id.into(),
						metadata.name,
						metadata.symbol,
						metadata.decimals,
					);
					match metadata_result {
						Ok(_) => {},
						Err(e) => {
							log::info!("error occurred while setting metadata for token: {:?}", e);

							return Err(Error::<T>::TokenCreateFailed.into())
						},
					}

					log::info!("minting token: {:?}", token_id);
					let mint_result = pallet_assets::Pallet::<T>::mint_into(
						token_id.into(),
						&org_account_id,
						min_balance,
					);
					match mint_result {
						Ok(_) => {},
						Err(e) => {
							log::info!("error occurred while minting token: {:?}", e);

							return Err(Error::<T>::TokenCreateFailed.into())
						},
					}
				}
			}

			if has_token_id.is_none() {
				if let Some(id) = organization.token_id {
					let token_id = Self::u32_to_asset_id(id);
					has_token_id = Some(token_id);

					let issuance = pallet_assets::Pallet::<T>::total_issuance(token_id.into())
						.try_into()
						.unwrap_or(0u128);

					if issuance == 0 {
						return Err(Error::<T>::TokenNotExists.into())
					}
				}
			}

			if has_token_id.is_none() {
				return Err(Error::<T>::TokenNotProvided.into())
			}

			let org = Organization {
				founder: who.clone(),
				account_id: org_account_id,
				token_id: has_token_id.unwrap().into(),
				next_proposal_id: 0,
				config: OrganizationConfig {
					name: org_name,
					purpose: org_purpose,
					metadata: org_metadata,
				},
			};

			Organizations::<T>::insert(org_id, org);
			<NextOrgId<T>>::put(org_id.checked_add(1).unwrap());

			Self::deposit_event(Event::OrgRegistered(org_id, who));

			Ok(())
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

					org.next_proposal_id = proposal_id.checked_add(1).unwrap();
					Organizations::<T>::insert(org_id, org);
				},
				None => return Err(Error::<T>::OrganizationNotExist.into()),
			}

			Self::deposit_event(Event::ProposalAdded(org_id, who));

			Ok(())
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

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn account_id(app_id: u32) -> T::AccountId {
			T::PalletId::get().into_sub_account_truncating(app_id)
		}

		fn u32_to_asset_id(asset_id: u32) -> AssetId<T> {
			asset_id.try_into().ok().unwrap()
		}

		fn u128_to_balance(cost: u128) -> Balance<T> {
			TryInto::<Balance<T>>::try_into(cost).ok().unwrap()
		}

		// TODO: rework
		fn u128_to_balance_of(cost: u128) -> BalanceOf<T> {
			TryInto::<BalanceOf<T>>::try_into(cost).ok().unwrap()
		}
	}
}
