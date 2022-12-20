#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, HasCompact};
use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	pallet_prelude::*,
	traits::{
		tokens::fungibles::{metadata::Mutate as MetadataMutate, Create, Inspect, Mutate},
		Currency, Get, ReservableCurrency,
	},
	BoundedVec, PalletId,
};
pub use pallet::*;
use scale_info::{prelude::*, TypeInfo};
use serde::{self, Deserialize};
use sp_core::crypto::KeyTypeId;
use sp_io::offchain_index;
use sp_runtime::{
	offchain,
	traits::{AccountIdConversion, AtLeast32BitUnsigned, BlockNumberProvider, StaticLookup},
	transaction_validity::{
		InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction,
	},
};
use sp_std::{prelude::*, str};

use dao_primitives::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

type DaoOf<T> = Dao<
	<T as frame_system::Config>::AccountId,
	<T as Config>::AssetId,
	BoundedVec<u8, <T as Config>::DaoStringLimit>,
	BoundedVec<u8, <T as Config>::DaoStringLimit>,
>;
type PolicyOf = DaoPolicy;

type AssetId<T> = <T as Config>::AssetId;
type Balance<T> = <T as Config>::Balance;

pub type BlockNumber = u32;

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"sctl");

const UNSIGNED_TXS_PRIORITY: u64 = 100;

const HTTP_REMOTE_REQUEST: &str =
	"https://celo-mainnet.infura.io/v3/9e0a8afb9be04831b75066778842ac8c";
const ERC20_TOKEN_TOTAL_SUPPLY_SIGNATURE: &str =
	"0x70a08231000000000000000000000000d8da6bf26964af9d7eed9e03e53415d37aa96045";

const FETCH_TIMEOUT_PERIOD: u64 = 3000; // in milli-seconds
const LOCK_TIMEOUT_EXPIRATION: u64 = FETCH_TIMEOUT_PERIOD + 1000; // in milli-seconds
const LOCK_BLOCK_EXPIRATION: u32 = 3; // in block number

const ONCHAIN_TX_KEY: &[u8] = b"societal-dao::storage::tx";

pub mod crypto {
	use crate::KEY_TYPE;
	use sp_core::sr25519::Signature as Sr25519Signature;
	use sp_runtime::{
		app_crypto::{app_crypto, sr25519},
		traits::Verify,
		MultiSignature, MultiSigner,
	};

	app_crypto!(sr25519, KEY_TYPE);

	pub struct TestAuthId;
	// implemented for societal-node-runtime
	impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for TestAuthId {
		type RuntimeAppPublic = Public;
		type GenericSignature = sp_core::sr25519::Signature;
		type GenericPublic = sp_core::sr25519::Public;
	}

	// implemented for mock runtime in test
	impl frame_system::offchain::AppCrypto<<Sr25519Signature as Verify>::Signer, Sr25519Signature>
		for TestAuthId
	{
		type RuntimeAppPublic = Public;
		type GenericSignature = sp_core::sr25519::Signature;
		type GenericPublic = sp_core::sr25519::Public;
	}
}

#[derive(Deserialize, Encode, Decode)]
struct InfuraResponse {
	#[serde(deserialize_with = "de_string_to_bytes")]
	result: Vec<u8>,
}

#[derive(Debug, Deserialize, Encode, Decode, Default)]
struct IndexingData(Vec<u8>, (u32, Vec<u8>));

#[frame_support::pallet]
pub mod pallet {
	pub use super::*;
	use frame_support::{pallet_prelude::*, traits::fungibles::Transfer};
	use frame_system::{
		offchain::{AppCrypto, CreateSignedTransaction, SubmitTransaction},
		pallet_prelude::*,
	};
	use serde_json::json;
	use sp_runtime::offchain::{
		storage::StorageValueRef,
		storage_lock::{BlockAndTime, StorageLock},
	};

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(4);

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + CreateSignedTransaction<Call<Self>> {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type Currency: ReservableCurrency<Self::AccountId>;

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
		type DaoTokenMinBalanceLimit: Get<u128>;

		#[pallet::constant]
		type DaoTokenBalanceLimit: Get<u128>;

		#[pallet::constant]
		type DaoTokenVotingMinThreshold: Get<u128>;

		#[pallet::constant]
		type ExpectedBlockTime: Get<u64>;

		type CouncilProvider: InitializeDaoMembers<u32, Self::AccountId>
			+ ContainsDaoMember<u32, Self::AccountId>;

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
			> + Transfer<
				Self::AccountId,
				AssetId = <Self as pallet::Config>::AssetId,
				Balance = <Self as pallet::Config>::Balance,
			>;

		/// The identifier type for an offchain worker.
		type AuthorityId: AppCrypto<Self::Public, Self::Signature>;
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
		StorageMap<_, Blake2_128Concat, u32, PolicyOf, OptionQuery>;

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn offchain_worker(block_number: T::BlockNumber) {
			log::info!("Entering off-chain worker");

			let key = Self::derived_key(block_number);
			let storage_ref = StorageValueRef::persistent(&key);

			if let Ok(Some(data)) = storage_ref.get::<IndexingData>() {
				log::info!(
					"off-chain indexing data: {:?}, {:?}",
					str::from_utf8(&data.0).unwrap_or("error"),
					data.1
				);

				let dao_id = data.1 .0;
				let token_address = data.1 .1;

				let result = Self::fetch_token_total_supply(token_address);
				if let Err(e) = result {
					log::error!("offchain_worker error: {:?}", e);

					//TODO: disapprove dao - remove?

					return
				}

				let call = Call::approve_dao { dao_id };

				let result =
					SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
						.map_err(|_| {
							log::error!("Failed in offchain_unsigned_tx");
							<Error<T>>::OffchainUnsignedTxError
						});

				if let Err(_) = result {
					log::error!("dao approval error: {:?}", dao_id);
				}
			} else {
				log::info!("no off-chain indexing data retrieved.");
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
				Call::approve_dao { dao_id } => valid_tx(b"approve_dao".to_vec()),
				_ => InvalidTransaction::Call.into(),
			}
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		DaoRegistered(u32, T::AccountId),
		DaoApproved(u32),
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
		TokenBalanceInvalid,
		TokenBalanceLow,
		TokenTransferFailed,
		TokenAddressInvalid,
		InvalidInput,
		PolicyNotExist,
		// Error returned when fetching http
		HttpFetchingError,
		OffchainUnsignedTxError,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		// TODO: calculate dynamic weight
		#[pallet::weight(10_000 + T::DbWeight::get().writes(4).ref_time())]
		pub fn create_dao(
			origin: OriginFor<T>,
			council: Vec<<T::Lookup as StaticLookup>::Source>,
			data: Vec<u8>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			let dao = serde_json::from_slice::<DaoPayload>(&data)
				.map_err(|_| Error::<T>::InvalidInput)?;

			let dao_id = <NextDaoId<T>>::get();
			let dao_account_id = Self::account_id(dao_id);

			let dao_name = BoundedVec::<u8, T::DaoStringLimit>::try_from(dao.name.clone())
				.map_err(|_| Error::<T>::NameTooLong)?;

			let dao_purpose = BoundedVec::<u8, T::DaoStringLimit>::try_from(dao.purpose.clone())
				.map_err(|_| Error::<T>::PurposeTooLong)?;

			let dao_metadata = BoundedVec::<u8, T::DaoStringLimit>::try_from(dao.metadata.clone())
				.map_err(|_| Error::<T>::MetadataTooLong)?;

			let min = <T as Config>::Currency::minimum_balance();
			let _ = <T as Config>::Currency::make_free_balance_be(&dao_account_id, min);

			// setting submitter as a Dao council by default
			let mut council_members: Vec<T::AccountId> = vec![who.clone()];
			for member in council {
				let account = T::Lookup::lookup(member)?;
				if council_members.contains(&account) {
					continue
				}

				council_members.push(account);
			}

			let mut has_token_id: Option<AssetId<T>> = None;
			let mut has_token_address: Option<BoundedVec<u8, <T as Config>::DaoStringLimit>> = None;
			let mut dao_status: DaoStatus = DaoStatus::Pending;

			if let Some(token) = dao.token {
				let token_id = Self::u32_to_asset_id(token.token_id);
				has_token_id = Some(token_id);

				let metadata = token.metadata;

				if token.min_balance == 0 || token.min_balance > T::DaoTokenBalanceLimit::get() {
					return Err(Error::<T>::TokenBalanceInvalid.into())
				}

				let min_balance = Self::u128_to_balance(T::DaoTokenMinBalanceLimit::get());

				let issuance =
					T::AssetProvider::total_issuance(token_id).try_into().unwrap_or(0u128);

				if issuance > 0 {
					return Err(Error::<T>::TokenAlreadyExists.into())
				} else {
					T::AssetProvider::create(token_id, dao_account_id.clone(), false, min_balance)
						.map_err(|_| Error::<T>::TokenCreateFailed)?;

					T::AssetProvider::set(
						token_id,
						&dao_account_id,
						metadata.name,
						metadata.symbol,
						metadata.decimals,
					)
					.map_err(|_| Error::<T>::TokenCreateFailed)?;

					T::AssetProvider::mint_into(
						token_id,
						&dao_account_id,
						Self::u128_to_balance(
							T::DaoTokenBalanceLimit::get().min(token.min_balance),
						),
					)
					.map_err(|_| Error::<T>::TokenCreateFailed)?;

					for member in council_members.clone() {
						T::AssetProvider::transfer(
							token_id,
							&dao_account_id,
							&member,
							Self::u128_to_balance(T::DaoTokenVotingMinThreshold::get()),
							true,
						)
						.map_err(|_| Error::<T>::TokenTransferFailed)?;
					}
				}

				dao_status = DaoStatus::Success;
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

					dao_status = DaoStatus::Success;
				} else if let Some(token_address) = dao.token_address {
					let token_address =
						BoundedVec::<u8, T::DaoStringLimit>::try_from(token_address)
							.map_err(|_| Error::<T>::TokenAddressInvalid)?;
					has_token_address = Some(token_address.clone());

					let key = Self::derived_key(frame_system::Pallet::<T>::block_number());
					let data: IndexingData =
						IndexingData(b"approve_dao".to_vec(), (dao_id, Vec::from(token_address)));

					offchain_index::set(&key, &data.encode());
				}
			}

			if has_token_id.is_none() && has_token_address.is_none() {
				return Err(Error::<T>::TokenNotProvided.into())
			}

			// TODO
			let policy = DaoPolicy {
				proposal_bond: dao.policy.proposal_bond,
				proposal_bond_min: dao.policy.proposal_bond_min,
				proposal_bond_max: None,
				proposal_period: dao.policy.proposal_period /
					T::ExpectedBlockTime::get() as BlockNumber,
				approve_origin: dao.policy.approve_origin,
				reject_origin: dao.policy.reject_origin,
				token_voting_min_threshold: T::DaoTokenVotingMinThreshold::get(),
			};
			Policies::<T>::insert(dao_id, policy);

			let dao = Dao {
				founder: who.clone(),
				account_id: dao_account_id,
				token_id: has_token_id,
				token_address: has_token_address,
				status: dao_status,
				config: DaoConfig { name: dao_name, purpose: dao_purpose, metadata: dao_metadata },
			};
			Daos::<T>::insert(dao_id, dao);

			T::CouncilProvider::initialize_members(dao_id, council_members)?;

			<NextDaoId<T>>::put(dao_id.checked_add(1).unwrap());

			Self::deposit_event(Event::DaoRegistered(dao_id, who));

			Ok(())
		}

		#[pallet::weight(10_000)]
		pub fn approve_dao(origin: OriginFor<T>, dao_id: u32) -> DispatchResult {
			ensure_none(origin)?;

			let dao_option = Daos::<T>::get(dao_id);

			if dao_option.is_none() {
				return Err(Error::<T>::DaoNotExist.into())
			}

			let mut dao = dao_option.unwrap();
			dao.status = DaoStatus::Success;

			Daos::<T>::insert(dao_id, dao);

			Self::deposit_event(Event::DaoApproved(dao_id));

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

		pub fn u128_to_balance(cost: u128) -> Balance<T> {
			TryInto::<Balance<T>>::try_into(cost).ok().unwrap()
		}

		fn u128_to_balance_of(cost: u128) -> BalanceOf<T> {
			TryInto::<BalanceOf<T>>::try_into(cost).ok().unwrap()
		}

		#[deny(clippy::clone_double_ref)]
		fn derived_key(block_number: T::BlockNumber) -> Vec<u8> {
			block_number.using_encoded(|encoded_bn| {
				ONCHAIN_TX_KEY
					.iter()
					.chain(b"/".iter())
					.chain(encoded_bn)
					.copied()
					.collect::<Vec<u8>>()
			})
		}

		fn fetch_token_total_supply(token_address: Vec<u8>) -> Result<(), Error<T>> {
			let s_info = StorageValueRef::persistent(b"societal-node::eth-token-total-supply");

			if let Ok(Some(_infura_response)) = s_info.get::<InfuraResponse>() {
				return Ok(())
			}

			let mut lock = StorageLock::<BlockAndTime<Self>>::with_block_and_time_deadline(
				b"societal-dao::lock",
				LOCK_BLOCK_EXPIRATION,
				offchain::Duration::from_millis(LOCK_TIMEOUT_EXPIRATION),
			);

			if let Ok(_guard) = lock.try_lock() {
				let to = str::from_utf8(&token_address[..]).expect("Failed to serialize");
				let json = json!({
					"jsonrpc": "2.0",
					"method": "eth_call",
					"id": 1,
					"params": [
						{
							"to": to,
							"data": ERC20_TOKEN_TOTAL_SUPPLY_SIGNATURE
						},
						"latest"
					]
				});
				let body = &serde_json::to_vec(&json).expect("Failed to serialize")[..];

				match Self::fetch_n_parse(vec![body]) {
					Ok(infura_response) => {
						s_info.set(&infura_response);
					},
					Err(err) => return Err(err),
				}
			}
			Ok(())
		}

		fn fetch_n_parse(body: Vec<&[u8]>) -> Result<InfuraResponse, Error<T>> {
			let resp_bytes = Self::fetch_from_remote(body).map_err(|e| {
				log::error!("fetch_from_remote error: {:?}", e);
				<Error<T>>::HttpFetchingError
			})?;

			let resp_str =
				str::from_utf8(&resp_bytes).map_err(|_| <Error<T>>::HttpFetchingError)?;

			let infura_response: InfuraResponse =
				serde_json::from_str(resp_str).map_err(|_| <Error<T>>::HttpFetchingError)?;

			log::info!("info: {:?}", infura_response.result);

			Ok(infura_response)
		}

		fn fetch_from_remote(body: Vec<&[u8]>) -> Result<Vec<u8>, Error<T>> {
			let request = offchain::http::Request::post(HTTP_REMOTE_REQUEST, body);

			// Keeping the offchain worker execution time reasonable, so limiting the call to be
			// within 3s.
			let timeout = sp_io::offchain::timestamp()
				.add(offchain::Duration::from_millis(FETCH_TIMEOUT_PERIOD));

			let pending = request
				.deadline(timeout) // Setting the timeout time
				.send() // Sending the request out by the host
				.map_err(|_| <Error<T>>::HttpFetchingError)?;

			let response = pending
				.try_wait(timeout)
				.map_err(|_| <Error<T>>::HttpFetchingError)?
				.map_err(|_| <Error<T>>::HttpFetchingError)?;

			if response.code != 200 {
				log::error!("Unexpected http request status code: {}", response.code);
				return Err(<Error<T>>::HttpFetchingError)
			}

			Ok(response.body().collect::<Vec<u8>>())
		}
	}
}

impl<T: Config> DaoProvider for Pallet<T> {
	type Id = u32;
	type AccountId = T::AccountId;
	type AssetId = T::AssetId;
	type Policy = PolicyOf;

	fn exists(id: Self::Id) -> Result<(), DispatchError> {
		if !Daos::<T>::contains_key(&id) {
			return Err(Error::<T>::DaoNotExist.into())
		}

		Ok(())
	}

	fn count() -> u32 {
		NextDaoId::<T>::get()
	}

	fn policy(id: Self::Id) -> Result<Self::Policy, DispatchError> {
		match Policies::<T>::get(&id) {
			Some(policy) => Ok(policy),
			None => Err(Error::<T>::PolicyNotExist.into()),
		}
	}

	fn ensure_member(id: Self::Id, who: &T::AccountId) -> Result<bool, DispatchError> {
		T::CouncilProvider::contains(id, who)
	}

	fn ensure_token_balance(id: Self::Id, who: &Self::AccountId) -> Result<(), DispatchError> {
		let dao = Daos::<T>::get(id).ok_or(Error::<T>::DaoNotExist)?;

		// TODO: rework
		if dao.token_id.is_some() {
			if T::AssetProvider::balance(dao.token_id.unwrap(), who) <
				Self::u128_to_balance(T::DaoTokenVotingMinThreshold::get())
			{
				return Err(Error::<T>::TokenBalanceLow.into())
			}
		} else if dao.token_address.is_some() {
			// TODO

			return Ok(())
		}

		Ok(())
	}

	fn dao_account_id(id: Self::Id) -> Self::AccountId {
		Self::account_id(id)
	}
}

impl<T: Config> BlockNumberProvider for Pallet<T> {
	type BlockNumber = T::BlockNumber;
	fn current_block_number() -> Self::BlockNumber {
		<frame_system::Pallet<T>>::block_number()
	}
}
