#![cfg_attr(not(feature = "std"), no_std)]

use codec::HasCompact;
use frame_support::{
	dispatch::DispatchResult,
	traits::{
		tokens::fungibles::{metadata::Mutate as MetadataMutate, Create, Inspect, Mutate},
		Currency,
		ExistenceRequirement::KeepAlive,
		Get, ReservableCurrency, UnfilteredDispatchable, UnixTime,
	},
	weights::GetDispatchInfo,
	BoundedVec, PalletId,
};
pub use pallet::*;
use scale_info::TypeInfo;
use serde::{self};
use sp_runtime::traits::{AccountIdConversion, AtLeast32BitUnsigned, StaticLookup};
use sp_std::prelude::*;

use dao_primitives::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

// TODO
type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

type DaoOf<T> = Dao<
	<T as frame_system::Config>::AccountId,
	<T as Config>::AssetId,
	BoundedVec<u8, <T as Config>::DaoStringLimit>,
	BoundedVec<u8, <T as Config>::DaoStringLimit>,
>;
// TODO
type PolicyOf<T> = DaoPolicy<<T as frame_system::Config>::AccountId>;

type AssetId<T> = <T as Config>::AssetId;
type Balance<T> = <T as Config>::Balance;

pub type BlockNumber = u32;

#[frame_support::pallet]
pub mod pallet {
	pub use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(4);

	// const MAX_ACCOUNT_LIMIT: u32 = 10;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type Call: Parameter + UnfilteredDispatchable<Origin = Self::Origin> + GetDispatchInfo;
		type Currency: ReservableCurrency<Self::AccountId>;
		type SupervisorOrigin: EnsureOrigin<Self::Origin>;
		type TimeProvider: UnixTime;

		type AssetId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo
			+ From<u32>
			+ Ord;

		type Balance: Member
			+ Parameter
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo
			+ From<u128>
			+ Ord;

		#[pallet::constant]
		type PalletId: Get<PalletId>;

		#[pallet::constant]
		type DaoStringLimit: Get<u32>;

		#[pallet::constant]
		type DaoMetadataLimit: Get<u32>;

		#[pallet::constant]
		type ExpectedBlockTime: Get<u64>;

		// TODO: rework providers
		type CouncilProvider: InitializeDaoMembers<u32, Self::AccountId>;

		type AssetProvider: Inspect<
				Self::AccountId,
				AssetId = <Self as pallet::Config>::AssetId,
				Balance = <Self as pallet::Config>::Balance,
			> + Create<
				Self::AccountId,
				AssetId = <Self as pallet::Config>::AssetId,
				Balance = <Self as pallet::Config>::Balance,
			> + MetadataMutate<
				Self::AccountId,
				AssetId = <Self as pallet::Config>::AssetId,
				Balance = <Self as pallet::Config>::Balance,
			> + Mutate<
				Self::AccountId,
				AssetId = <Self as pallet::Config>::AssetId,
				Balance = <Self as pallet::Config>::Balance,
			>;
	}

	#[pallet::storage]
	#[pallet::getter(fn next_dao_id)]
	pub(super) type NextDaoId<T> = StorageValue<_, u32, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn daos)]
	pub(super) type Daos<T: Config> = StorageMap<_, Blake2_128Concat, u32, DaoOf<T>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn policies)]
	pub(super) type Policies<T: Config> =
		StorageMap<_, Blake2_128Concat, u32, PolicyOf<T>, OptionQuery>;

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event documentation should end with an array that provides descriptive names for event
		/// parameters. [something, who]
		DaoRegistered(u32, T::AccountId),
		DaoJoined(u32, T::AccountId),
		ProposalAdded(u32, T::AccountId),
	}

	#[pallet::error]
	pub enum Error<T> {
		NoneValue,
		DaoNotExist,
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
		pub fn create_dao(
			origin: OriginFor<T>,
			// additional council member - should be multi-account lookup or something
			council: Vec<<T::Lookup as StaticLookup>::Source>,
			data: Vec<u8>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			let payload = serde_json::from_slice::<DaoPayload>(&data);
			if payload.is_err() {
				log::info!("err: {:?}", payload);

				return Err(Error::<T>::InvalidInput.into())
			}

			let dao = payload.unwrap();

			let dao_id = <NextDaoId<T>>::get();
			let dao_account_id = Self::account_id(dao_id);

			let mut dao_name = Default::default();
			match BoundedVec::<u8, T::DaoStringLimit>::try_from(dao.name.clone()) {
				Ok(name) => {
					dao_name = name;
				},
				Err(_) => return Err(Error::<T>::DaoNotExist.into()),
			}

			let mut dao_purpose = Default::default();
			match BoundedVec::<u8, T::DaoStringLimit>::try_from(dao.purpose.clone()) {
				Ok(purpose) => {
					dao_purpose = purpose;
				},
				Err(_) => return Err(Error::<T>::DaoNotExist.into()),
			}

			let mut dao_metadata = Default::default();
			match BoundedVec::<u8, T::DaoStringLimit>::try_from(dao.metadata.clone()) {
				Ok(metadata) => {
					dao_metadata = metadata;
				},
				Err(_) => return Err(Error::<T>::DaoNotExist.into()),
			}

			let min = <T as Config>::Currency::minimum_balance();
			let _ = <T as Config>::Currency::make_free_balance_be(&dao_account_id, min);

			// reserving some deposit for token metadata storage
			let _ = <T as Config>::Currency::transfer(
				&who,
				&dao_account_id,
				Self::u128_to_balance_of(2_000_000_000_000_000), //TODO: should be dynamic
				KeepAlive,
			);

			let mut has_token_id: Option<AssetId<T>> = None;

			if let Some(token) = dao.token {
				let token_id = Self::u32_to_asset_id(token.token_id);
				has_token_id = Some(token_id);

				let metadata = token.metadata;
				let min_balance = Self::u128_to_balance(token.min_balance).into();

				let issuance =
					T::AssetProvider::total_issuance(token_id).try_into().unwrap_or(0u128);

				if issuance > 0 {
					return Err(Error::<T>::TokenAlreadyExists.into())
				} else {
					log::info!("issuing token: {:?}", token_id);

					let create_result = T::AssetProvider::create(
						token_id,
						dao_account_id.clone(),
						false,
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
					let metadata_result = T::AssetProvider::set(
						token_id,
						&dao_account_id.clone(),
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
					let mint_result =
						T::AssetProvider::mint_into(token_id, &dao_account_id, min_balance);
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
				if let Some(id) = dao.token_id {
					let token_id = Self::u32_to_asset_id(id);
					has_token_id = Some(token_id);

					let issuance =
						T::AssetProvider::total_issuance(token_id).try_into().unwrap_or(0u128);

					if issuance == 0 {
						return Err(Error::<T>::TokenNotExists.into())
					}
				}
			}

			if has_token_id.is_none() {
				return Err(Error::<T>::TokenNotProvided.into())
			}

			// TODO
			let policy = DaoPolicy {
				proposal_bond: dao.policy.proposal_bond,
				proposal_bond_min: dao.policy.proposal_bond_min,
				proposal_bond_max: None,
				proposal_period: dao.policy.proposal_period /
					T::ExpectedBlockTime::get() as BlockNumber,
				prime_account: who.clone(),
				approve_origin: (3, 5),
				reject_origin: (1, 2),
				add_origin: (1, 2),
				remove_origin: (1, 2),
				swap_origin: (1, 2),
				reset_origin: (1, 2),
				prime_origin: (1, 2),
			};
			Policies::<T>::insert(dao_id, policy);

			let dao = Dao {
				founder: who.clone(),
				account_id: dao_account_id,
				token_id: has_token_id.unwrap().into(),
				config: DaoConfig { name: dao_name, purpose: dao_purpose, metadata: dao_metadata },
			};
			Daos::<T>::insert(dao_id, dao);

			// setting submitter as a Dao council by default
			let mut council_members: Vec<T::AccountId> = vec![who.clone()];
			for member in council {
				council_members.push(T::Lookup::lookup(member)?);
			}
			T::CouncilProvider::initialize_members(dao_id, council_members)?;

			<NextDaoId<T>>::put(dao_id.checked_add(1).unwrap());

			Self::deposit_event(Event::DaoRegistered(dao_id, who));

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn account_id(dao_id: u32) -> T::AccountId {
			T::PalletId::get().into_sub_account_truncating(dao_id)
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

impl<T: Config> DaoProvider for Pallet<T> {
	type Id = u32;
	type AccountId = T::AccountId;
	type Policy = PolicyOf<T>;

	fn exists(id: Self::Id) -> bool {
		Daos::<T>::contains_key(&id)
	}

	fn count() -> u32 {
		NextDaoId::<T>::get()
	}

	fn policy(id: Self::Id) -> Option<Self::Policy> {
		Policies::<T>::get(&id)
	}

	fn dao_account_id(id: Self::Id) -> Self::AccountId {
		Self::account_id(id)
	}
}
