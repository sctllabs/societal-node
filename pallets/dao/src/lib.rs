#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, HasCompact};
use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	pallet_prelude::*,
	traits::{
		tokens::fungibles::{metadata::Mutate as MetadataMutate, Create, Inspect, Mutate},
		Currency, EnsureOriginWithArg, Get, LockIdentifier, ReservableCurrency,
	},
	BoundedVec, PalletId,
};
use frame_system::pallet_prelude::OriginFor;
pub use pallet::*;
use scale_info::{prelude::*, TypeInfo};
use serde::{self, Deserialize};
use sp_core::crypto::KeyTypeId;
use sp_io::offchain_index;
use sp_runtime::{
	traits::{AccountIdConversion, AtLeast32BitUnsigned, BlockNumberProvider, Saturating},
	transaction_validity::{
		InvalidTransaction, TransactionSource, TransactionValidity, ValidTransaction,
	},
};
use sp_std::{prelude::*, str};

use dao_primitives::*;
use eth_primitives::EthRpcProvider;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

type DaoOf<T> = Dao<
	<T as frame_system::Config>::AccountId,
	<T as Config>::AssetId,
	BoundedVec<u8, <T as Config>::DaoNameLimit>,
	BoundedVec<u8, <T as Config>::DaoStringLimit>,
	BoundedVec<u8, <T as Config>::DaoMetadataLimit>,
>;
type PolicyOf = DaoPolicy;

type PendingDaoOf<T> = PendingDao<
	<T as frame_system::Config>::AccountId,
	<T as Config>::AssetId,
	BoundedVec<u8, <T as Config>::DaoNameLimit>,
	BoundedVec<u8, <T as Config>::DaoStringLimit>,
	BoundedVec<u8, <T as Config>::DaoMetadataLimit>,
	BoundedVec<<T as frame_system::Config>::AccountId, <T as Config>::DaoMaxCouncilMembers>,
	BoundedVec<
		<T as frame_system::Config>::AccountId,
		<T as Config>::DaoMaxTechnicalCommitteeMembers,
	>,
	<T as frame_system::Config>::BlockNumber,
>;

pub type CallOf<T> = <T as Config>::RuntimeCall;

/// Dao ID. Just a `u32`.
pub type DaoId = u32;

/// Token Supply
pub type TokenSupply = u128;

type AssetId<T> = <T as Config>::AssetId;
type Balance<T> = <T as Config>::Balance;

pub type BlockNumber = u32;

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"sctl");

const UNSIGNED_TXS_PRIORITY: u64 = 100;

const ONCHAIN_TX_KEY: &[u8] = b"societal-dao::storage::tx";

const TOKEN_MIN_BALANCE: u128 = 1;

const DAO_FACTORY_ID: LockIdentifier = *b"daofctry";
const DAO_SPEND_ID: [u8; 4] = *b"spnd";
const DAO_REFERENDUM_LAUNCH_ID: [u8; 4] = *b"refl";
const DAO_REFERENDUM_BAKE_ID: [u8; 4] = *b"refb";

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

#[derive(Debug, Encode, Decode, Clone, Default, Deserialize, PartialEq, Eq)]
pub enum OffchainData<Hash> {
	#[default]
	Default,
	ApproveDao {
		dao_hash: Hash,
		token_address: Vec<u8>,
	},
}

#[derive(Debug, Deserialize, Encode, Decode, Default)]
struct IndexingData<Hash>(Vec<u8>, OffchainData<Hash>);

#[frame_support::pallet]
pub mod pallet {
	pub use super::*;
	use crate::weights::WeightInfo;
	use eth_primitives::EthRpcService;
	use frame_support::traits::{
		fungibles::Transfer,
		schedule::{
			v3::{Named as ScheduleNamed, TaskName},
			DispatchTime,
		},
		QueryPreimage, StorePreimage,
	};
	use frame_system::{
		offchain::{AppCrypto, CreateSignedTransaction, SubmitTransaction},
		pallet_prelude::*,
	};
	use sp_runtime::{
		offchain::storage::StorageValueRef,
		traits::{Dispatchable, Hash, Zero},
	};

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(4);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + CreateSignedTransaction<Call<Self>> {
		type RuntimeCall: Parameter
			+ Dispatchable<RuntimeOrigin = Self::RuntimeOrigin>
			+ From<Call<Self>>
			+ IsType<<Self as frame_system::Config>::RuntimeCall>
			+ From<frame_system::Call<Self>>;

		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

		type AssetId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo
			+ From<u128>
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
		type DaoNameLimit: Get<u32>;

		#[pallet::constant]
		type DaoStringLimit: Get<u32>;

		#[pallet::constant]
		type DaoMetadataLimit: Get<u32>;

		#[pallet::constant]
		type DaoMaxCouncilMembers: Get<u32>;

		#[pallet::constant]
		type DaoMaxTechnicalCommitteeMembers: Get<u32>;

		#[pallet::constant]
		type DaoMinTreasurySpendPeriod: Get<u32>;

		type CouncilProvider: InitializeDaoMembers<u32, Self::AccountId>
			+ ContainsDaoMember<u32, Self::AccountId>;

		type TechnicalCommitteeProvider: InitializeDaoMembers<u32, Self::AccountId>
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
			> + Transfer<Self::AccountId>;

		/// The identifier type for an offchain worker.
		type AuthorityId: AppCrypto<Self::Public, Self::Signature>;

		type OffchainEthService: EthRpcService;

		type ApproveOrigin: EnsureOriginWithArg<Self::RuntimeOrigin, DaoOrigin<Self::AccountId>>;

		type Scheduler: ScheduleNamed<Self::BlockNumber, CallOf<Self>, Self::PalletsOrigin>;

		/// Overarching type of all pallets origins.
		type PalletsOrigin: From<RawOrigin<Self::AccountId>>;

		type Preimages: QueryPreimage + StorePreimage;

		/// Runtime hooks to external pallet using treasury to compute spend funds.
		type SpendDaoFunds: SpendDaoFunds<DaoId>;

		type DaoReferendumScheduler: DaoReferendumScheduler<DaoId>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;

		#[cfg(feature = "runtime-benchmarks")]
		type DaoReferendumBenchmarkHelper: DaoReferendumBenchmarkHelper<
			DaoId,
			Self::AccountId,
			CallOf<Self>,
			Self::Balance,
		>;
	}

	/// Origin for the dao pallet.
	#[pallet::origin]
	pub type Origin<T> = RawOrigin<<T as frame_system::Config>::AccountId>;

	#[pallet::storage]
	#[pallet::getter(fn next_dao_id)]
	pub(super) type NextDaoId<T> = StorageValue<_, u32, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn daos)]
	pub(super) type Daos<T: Config> = StorageMap<_, Blake2_128Concat, u32, DaoOf<T>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn pending_daos)]
	pub(super) type PendingDaos<T: Config> =
		StorageMap<_, Blake2_128Concat, T::Hash, PendingDaoOf<T>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn policies)]
	pub(super) type Policies<T: Config> =
		StorageMap<_, Blake2_128Concat, u32, PolicyOf, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn eth_rpc_url)]
	pub type EthRpcUrl<T> =
		StorageValue<_, BoundedVec<u8, <T as Config>::DaoStringLimit>, ValueQuery>;

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
				BoundedVec::<u8, T::DaoStringLimit>::try_from(self.eth_rpc_url.clone()).unwrap(),
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
				if let OffchainData::ApproveDao { dao_hash, token_address } = offchain_data.clone()
				{
					match T::OffchainEthService::parse_token_balance(
						T::OffchainEthService::fetch_token_total_supply(token_address, None),
					) {
						Ok(total_supply) => {
							call = Some(Call::approve_dao { dao_hash, approve: total_supply > 0 });
						},
						Err(e) => {
							log::error!("offchain_worker error: {:?}", e);

							call = Some(Call::approve_dao { dao_hash, approve: false });
						},
					}
				}

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
					if let OffchainData::ApproveDao { dao_hash, .. } = offchain_data {
						log::error!("dao approval error: {:?}", dao_hash);
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
				Call::approve_dao { .. } => valid_tx(b"approve_dao".to_vec()),
				_ => InvalidTransaction::Call.into(),
			}
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		DaoRegistered {
			dao_id: DaoId,
			founder: T::AccountId,
			account_id: T::AccountId,
			council: BoundedVec<T::AccountId, T::DaoMaxCouncilMembers>,
			technical_committee: BoundedVec<T::AccountId, T::DaoMaxTechnicalCommitteeMembers>,
			token: DaoToken<T::AssetId, BoundedVec<u8, T::DaoStringLimit>>,
			config: DaoConfig<
				BoundedVec<u8, T::DaoNameLimit>,
				BoundedVec<u8, T::DaoStringLimit>,
				BoundedVec<u8, T::DaoMetadataLimit>,
			>,
			policy: DaoPolicy,
		},
		DaoPendingApproval {
			dao_id: DaoId,
			founder: T::AccountId,
			account_id: T::AccountId,
			token: DaoToken<T::AssetId, BoundedVec<u8, T::DaoStringLimit>>,
			config: DaoConfig<
				BoundedVec<u8, T::DaoNameLimit>,
				BoundedVec<u8, T::DaoStringLimit>,
				BoundedVec<u8, T::DaoMetadataLimit>,
			>,
			policy: DaoPolicy,
		},
		DaoTokenTransferred {
			dao_id: DaoId,
			token_id: T::AssetId,
			beneficiary: T::AccountId,
			amount: Balance<T>,
		},
		DaoMetadataUpdated {
			dao_id: DaoId,
			metadata: Vec<u8>,
		},
		DaoPolicyUpdated {
			dao_id: DaoId,
			policy: DaoPolicy,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		NoneValue,
		DaoNotExist,
		NameTooLong,
		PurposeTooLong,
		MetadataTooLong,
		CouncilMembersOverflow,
		TechnicalCommitteeMembersOverflow,
		TokenNotProvided,
		TokenAlreadyExists,
		TokenNotExists,
		TokenCreateFailed,
		TokenMetadataFailed,
		TokenMintFailed,
		TokenBalanceInvalid,
		TokenBalanceLow,
		TokenTransferFailed,
		TokenAddressInvalid,
		InvalidInput,
		PolicyNotExist,
		OffchainUnsignedTxError,
		NotSupported,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(T::WeightInfo::create_dao())]
		#[pallet::call_index(0)]
		pub fn create_dao(
			origin: OriginFor<T>,
			council: Vec<T::AccountId>,
			technical_committee: Vec<T::AccountId>,
			data: Vec<u8>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			Self::do_create_dao(who, council, technical_committee, data)
		}

		#[pallet::weight(T::WeightInfo::approve_dao())]
		#[pallet::call_index(2)]
		pub fn approve_dao(
			origin: OriginFor<T>,
			dao_hash: T::Hash,
			approve: bool,
		) -> DispatchResult {
			ensure_none(origin)?;

			let PendingDao { dao, policy, council, technical_committee, .. } =
				match <PendingDaos<T>>::take(dao_hash) {
					None => return Err(Error::<T>::DaoNotExist.into()),
					Some(dao) => dao,
				};

			if approve {
				return Self::do_register_dao(dao, policy, council, technical_committee)
			}

			Ok(())
		}

		#[pallet::weight(T::WeightInfo::update_dao_metadata())]
		#[pallet::call_index(3)]
		pub fn update_dao_metadata(
			origin: OriginFor<T>,
			dao_id: DaoId,
			metadata: Vec<u8>,
		) -> DispatchResult {
			Self::ensure_approved(origin, dao_id)?;

			let dao_metadata = BoundedVec::<u8, T::DaoMetadataLimit>::try_from(metadata.clone())
				.map_err(|_| Error::<T>::MetadataTooLong)?;

			Daos::<T>::mutate(dao_id, |maybe_dao| {
				if let Some(dao) = maybe_dao {
					dao.config.metadata = dao_metadata.clone();
				}
			});

			Self::deposit_event(Event::DaoMetadataUpdated { dao_id, metadata });

			Ok(())
		}

		#[pallet::weight(T::WeightInfo::update_dao_policy())]
		#[pallet::call_index(4)]
		pub fn update_dao_policy(
			origin: OriginFor<T>,
			dao_id: DaoId,
			policy: Vec<u8>,
		) -> DispatchResult {
			Self::ensure_approved(origin, dao_id)?;

			let policy = serde_json::from_slice::<DaoPolicy>(&policy)
				.map_err(|_| Error::<T>::InvalidInput)?;

			let old_policy = Self::policy(dao_id)?;
			Policies::<T>::insert(dao_id, policy.clone());

			if old_policy.spend_period != policy.spend_period {
				Self::schedule_dao_events(dao_id, policy.clone(), true)?;
			}

			Self::deposit_event(Event::DaoPolicyUpdated { dao_id, policy });

			Ok(())
		}

		#[pallet::weight(T::WeightInfo::mint_dao_token())]
		#[pallet::call_index(5)]
		pub fn mint_dao_token(
			origin: OriginFor<T>,
			dao_id: DaoId,
			amount: T::Balance,
		) -> DispatchResult {
			Self::ensure_approved(origin, dao_id)?;

			let dao_account_id = Self::dao_account_id(dao_id);
			let dao_token = Self::dao_token(dao_id)?;

			match dao_token {
				DaoToken::FungibleToken(token_id) => {
					T::AssetProvider::mint_into(token_id, &dao_account_id, amount)
						.map_err(|_| Error::<T>::TokenMintFailed)?;

					Ok(())
				},
				DaoToken::EthTokenAddress(_) => Err(Error::<T>::NotSupported.into()),
			}
		}

		#[pallet::weight(T::WeightInfo::spend_dao_funds())]
		#[pallet::call_index(6)]
		pub fn spend_dao_funds(origin: OriginFor<T>, dao_id: DaoId) -> DispatchResult {
			Self::ensure_approved(origin, dao_id)?;

			T::SpendDaoFunds::spend_dao_funds(dao_id);

			Ok(())
		}

		#[pallet::weight(T::WeightInfo::launch_dao_referendum())]
		#[pallet::call_index(7)]
		pub fn launch_dao_referendum(origin: OriginFor<T>, dao_id: DaoId) -> DispatchResult {
			Self::ensure_approved(origin, dao_id)?;

			T::DaoReferendumScheduler::launch_referendum(dao_id)
		}

		#[pallet::weight(T::WeightInfo::bake_dao_referendum())]
		#[pallet::call_index(8)]
		pub fn bake_dao_referendum(origin: OriginFor<T>, dao_id: DaoId) -> DispatchResult {
			Self::ensure_approved(origin, dao_id)?;

			T::DaoReferendumScheduler::bake_referendum(dao_id)
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn account_id(dao_id: u32) -> T::AccountId {
			T::PalletId::get().into_sub_account_truncating(dao_id)
		}

		fn u128_to_asset_id(asset_id: u128) -> AssetId<T> {
			asset_id.try_into().ok().unwrap()
		}

		pub fn u128_to_balance(cost: u128) -> Balance<T> {
			TryInto::<Balance<T>>::try_into(cost).ok().unwrap()
		}

		fn spend_dao_funds_task_id(dao_id: DaoId) -> TaskName {
			(DAO_FACTORY_ID, DAO_SPEND_ID, dao_id).encode_into()
		}

		fn launch_dao_referendum_task_id(dao_id: DaoId) -> TaskName {
			(DAO_FACTORY_ID, DAO_REFERENDUM_LAUNCH_ID, dao_id).encode_into()
		}

		fn bake_dao_referendum_task_id(dao_id: DaoId) -> TaskName {
			(DAO_FACTORY_ID, DAO_REFERENDUM_BAKE_ID, dao_id).encode_into()
		}

		pub fn do_create_dao(
			founder: T::AccountId,
			council: Vec<T::AccountId>,
			technical_committee: Vec<T::AccountId>,
			data: Vec<u8>,
		) -> DispatchResult {
			let DaoPayload { name, purpose, metadata, token, token_id, token_address, policy } =
				serde_json::from_slice::<DaoPayload>(&data)
					.map_err(|_| Error::<T>::InvalidInput)?;

			let dao_id = <NextDaoId<T>>::get();
			let dao_account_id = Self::account_id(dao_id);

			let dao_name = BoundedVec::<u8, T::DaoNameLimit>::try_from(name)
				.map_err(|_| Error::<T>::NameTooLong)?;

			let dao_purpose = BoundedVec::<u8, T::DaoStringLimit>::try_from(purpose)
				.map_err(|_| Error::<T>::PurposeTooLong)?;

			let dao_metadata = BoundedVec::<u8, T::DaoMetadataLimit>::try_from(metadata)
				.map_err(|_| Error::<T>::MetadataTooLong)?;

			let min = <T as Config>::Currency::minimum_balance();
			let _ = <T as Config>::Currency::make_free_balance_be(&dao_account_id, min);

			// setting submitter as a Dao council by default
			let mut council_members: Vec<T::AccountId> = vec![founder.clone()];
			for account in council {
				if council_members.contains(&account) {
					continue
				}

				council_members.push(account);
			}

			let council = BoundedVec::<T::AccountId, T::DaoMaxCouncilMembers>::try_from(
				council_members.clone(),
			)
			.map_err(|_| Error::<T>::CouncilMembersOverflow)?;

			let mut technical_committee_members: Vec<T::AccountId> = vec![];
			for account in technical_committee.into_iter() {
				if technical_committee_members.contains(&account) {
					continue
				}

				technical_committee_members.push(account);
			}

			let technical_committee =
				BoundedVec::<T::AccountId, T::DaoMaxTechnicalCommitteeMembers>::try_from(
					technical_committee_members.clone(),
				)
				.map_err(|_| Error::<T>::CouncilMembersOverflow)?;

			let config = DaoConfig { name: dao_name, purpose: dao_purpose, metadata: dao_metadata };

			ensure!(policy.spend_period.0 >= T::DaoMinTreasurySpendPeriod::get(), "Value too low");

			let mut has_token_id: Option<AssetId<T>> = None;

			if let Some(token) = token {
				let token_id = Self::u128_to_asset_id(token.token_id);
				has_token_id = Some(token_id);

				let metadata = token.metadata;

				let token_min_balance: Balance<T> =
					Self::u128_to_balance(token.min_balance.unwrap_or(TOKEN_MIN_BALANCE));
				let token_initial_balance: Balance<T> =
					Self::u128_to_balance(token.initial_balance.unwrap_or(TOKEN_MIN_BALANCE));
				if token_min_balance.is_zero() ||
					token_initial_balance.is_zero() ||
					token_initial_balance < token_min_balance
				{
					return Err(Error::<T>::TokenBalanceInvalid.into())
				}

				let issuance =
					T::AssetProvider::total_issuance(token_id).try_into().unwrap_or(0u128);

				if issuance > 0 {
					return Err(Error::<T>::TokenAlreadyExists.into())
				} else {
					T::AssetProvider::create(
						token_id,
						dao_account_id.clone(),
						true,
						token_min_balance,
					)
					.map_err(|_| Error::<T>::TokenCreateFailed)?;

					T::AssetProvider::set(
						token_id,
						&dao_account_id,
						metadata.name,
						metadata.symbol,
						metadata.decimals,
					)
					.map_err(|_| Error::<T>::TokenMetadataFailed)?;

					if token_initial_balance.gt(&Self::u128_to_balance(0)) {
						T::AssetProvider::mint_into(
							token_id,
							&dao_account_id,
							token_initial_balance,
						)
						.map_err(|_| Error::<T>::TokenMintFailed)?;
					}
				}
			}

			if has_token_id.is_none() {
				if let Some(id) = token_id {
					let token_id = Self::u128_to_asset_id(id);
					has_token_id = Some(token_id);

					let issuance =
						T::AssetProvider::total_issuance(token_id).try_into().unwrap_or(0u128);

					if issuance == 0 {
						return Err(Error::<T>::TokenNotExists.into())
					}
				} else if let Some(token_address) = token_address {
					let address =
						BoundedVec::<u8, T::DaoStringLimit>::try_from(token_address.clone())
							.map_err(|_| Error::<T>::TokenAddressInvalid)?;

					let dao = Dao {
						founder,
						config,
						account_id: dao_account_id,
						token: DaoToken::EthTokenAddress(address),
						status: DaoStatus::Pending,
					};

					let dao_hash = T::Hashing::hash_of(&dao);

					let key = Self::derived_key(frame_system::Pallet::<T>::block_number());
					let data: IndexingData<T::Hash> = IndexingData(
						b"approve_dao".to_vec(),
						OffchainData::ApproveDao { dao_hash, token_address },
					);

					offchain_index::set(&key, &data.encode());

					let dao_event_source = dao.clone();

					PendingDaos::<T>::insert(
						dao_hash,
						PendingDao {
							dao,
							policy: policy.clone(),
							council,
							technical_committee,
							block_number: frame_system::Pallet::<T>::block_number(),
						},
					);

					Self::deposit_event(Event::DaoPendingApproval {
						dao_id,
						founder: dao_event_source.founder,
						account_id: dao_event_source.account_id,
						token: dao_event_source.token,
						config: dao_event_source.config,
						policy,
					});

					return Ok(())
				} else {
					return Err(Error::<T>::TokenNotProvided.into())
				}
			}

			let dao = Dao {
				founder,
				config,
				account_id: dao_account_id,
				token: DaoToken::FungibleToken(has_token_id.unwrap()),
				status: DaoStatus::Success,
			};

			Self::do_register_dao(dao, policy, council, technical_committee)
		}

		pub fn do_register_dao(
			mut dao: DaoOf<T>,
			policy: PolicyOf,
			council: BoundedVec<T::AccountId, T::DaoMaxCouncilMembers>,
			technical_committee: BoundedVec<T::AccountId, T::DaoMaxTechnicalCommitteeMembers>,
		) -> DispatchResult {
			let dao_id = <NextDaoId<T>>::get();

			dao.account_id = Self::account_id(dao_id);
			dao.status = DaoStatus::Success;

			let dao_event_source = dao.clone();

			Policies::<T>::insert(dao_id, policy.clone());

			Daos::<T>::insert(dao_id, dao);

			T::CouncilProvider::initialize_members(dao_id, council.to_vec())?;

			T::TechnicalCommitteeProvider::initialize_members(
				dao_id,
				technical_committee.to_vec(),
			)?;

			<NextDaoId<T>>::put(dao_id.checked_add(1).unwrap());

			Self::schedule_dao_events(dao_id, policy.clone(), false)?;

			Self::deposit_event(Event::DaoRegistered {
				dao_id,
				founder: dao_event_source.founder,
				account_id: dao_event_source.account_id,
				council,
				technical_committee,
				token: dao_event_source.token,
				config: dao_event_source.config,
				policy,
			});

			Ok(())
		}

		pub fn schedule_dao_events(
			dao_id: DaoId,
			policy: DaoPolicy,
			reschedule: bool,
		) -> DispatchResult {
			let now = frame_system::Pallet::<T>::block_number();
			let DaoPolicy { spend_period, governance, .. } = policy;
			let account_id = Self::account_id(dao_id);

			let spend_task_id = Self::spend_dao_funds_task_id(dao_id);
			if reschedule {
				// Canceling previously scheduled treasury spend task
				T::Scheduler::cancel_named(spend_task_id)?;
			}
			// Scheduling DAO treasury periodic spending
			let spend_call = CallOf::<T>::from(Call::spend_dao_funds { dao_id });
			T::Scheduler::schedule_named(
				spend_task_id,
				DispatchTime::At(now.saturating_add(spend_period.0.into())),
				Some((spend_period.0.into(), u32::MAX)),
				50,
				RawOrigin::Dao(account_id.clone()).into(),
				T::Preimages::bound(spend_call)?.transmute(),
			)?;

			if let Some(governance) = governance {
				match governance {
					DaoGovernance::GovernanceV1(governance) => {
						let GovernanceV1Policy { launch_period, voting_period, .. } = governance;

						let ref_launch_task_id = Self::launch_dao_referendum_task_id(dao_id);
						if reschedule {
							// Canceling previously scheduled referendum launch task
							T::Scheduler::cancel_named(ref_launch_task_id)?;
						}
						let ref_launch_call =
							CallOf::<T>::from(Call::launch_dao_referendum { dao_id });
						// Scheduling DAO periodic referendum launch
						T::Scheduler::schedule_named(
							ref_launch_task_id,
							DispatchTime::At(now.saturating_add(launch_period.into())),
							Some((launch_period.into(), u32::MAX)),
							50,
							RawOrigin::Dao(account_id.clone()).into(),
							T::Preimages::bound(ref_launch_call)?.transmute(),
						)?;

						let ref_bake_task_id = Self::bake_dao_referendum_task_id(dao_id);
						if reschedule {
							// Canceling previously scheduled referendum bake task
							T::Scheduler::cancel_named(ref_bake_task_id)?;
						}
						let ref_bake_call = CallOf::<T>::from(Call::bake_dao_referendum { dao_id });
						// Scheduling DAO periodic referendum voting tally up
						T::Scheduler::schedule_named(
							ref_bake_task_id,
							DispatchTime::At(now.saturating_add(voting_period.into())),
							Some((voting_period.into(), u32::MAX)),
							50,
							RawOrigin::Dao(account_id).into(),
							T::Preimages::bound(ref_bake_call)?.transmute(),
						)?;
					},
					DaoGovernance::OwnershipWeightedVoting => {},
				}
			}

			Ok(())
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
	}
}

impl<T: Config> DaoProvider<T::Hash> for Pallet<T> {
	type Id = u32;
	type AccountId = T::AccountId;
	type AssetId = T::AssetId;
	type Policy = PolicyOf;
	type Origin = OriginFor<T>;
	type ApproveOrigin = T::ApproveOrigin;
	type NFTCollectionId = u32;

	fn exists(id: Self::Id) -> Result<(), DispatchError> {
		if !Daos::<T>::contains_key(id) {
			return Err(Error::<T>::DaoNotExist.into())
		}

		Ok(())
	}

	fn count() -> u32 {
		NextDaoId::<T>::get()
	}

	fn policy(id: Self::Id) -> Result<Self::Policy, DispatchError> {
		match Policies::<T>::get(id) {
			Some(policy) => Ok(policy),
			None => Err(Error::<T>::PolicyNotExist.into()),
		}
	}

	fn ensure_member(id: Self::Id, who: &T::AccountId) -> Result<bool, DispatchError> {
		T::CouncilProvider::contains(id, who)
	}

	fn dao_account_id(id: Self::Id) -> Self::AccountId {
		Self::account_id(id)
	}

	fn dao_token(id: Self::Id) -> Result<DaoToken<Self::AssetId, Vec<u8>>, DispatchError> {
		match Daos::<T>::get(id) {
			None => Err(Error::<T>::DaoNotExist.into()),
			Some(dao) => Ok(match dao.token {
				DaoToken::FungibleToken(token_id) => DaoToken::FungibleToken(token_id),
				DaoToken::EthTokenAddress(token_address) =>
					DaoToken::EthTokenAddress(token_address.to_vec()),
			}),
		}
	}

	fn dao_nft_collection_id(
		_id: Self::Id,
	) -> Result<Option<Self::NFTCollectionId>, DispatchError> {
		Err(Error::<T>::NotSupported.into())
	}

	fn ensure_approved(
		origin: OriginFor<T>,
		dao_id: Self::Id,
	) -> DispatchResultWithDaoOrigin<Self::AccountId> {
		let dao_account_id = Self::dao_account_id(dao_id);
		let approve_origin = Self::policy(dao_id)?.approve_origin;
		let dao_origin = DaoOrigin { dao_account_id, proportion: approve_origin };

		Self::ApproveOrigin::ensure_origin(origin, &dao_origin)?;

		Ok(dao_origin)
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn create_dao(
		who: T::AccountId,
		council: Vec<T::AccountId>,
		technical_committee: Vec<T::AccountId>,
		data: Vec<u8>,
	) -> Result<(), DispatchError> {
		Self::do_create_dao(who, council, technical_committee, data)
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin(dao_origin: &DaoOrigin<Self::AccountId>) -> Result<Self::Origin, ()> {
		Self::ApproveOrigin::try_successful_origin(dao_origin)
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

pub struct EnsureDao<AccountId>(PhantomData<AccountId>);
impl<
		O: Into<Result<RawOrigin<AccountId>, O>> + From<RawOrigin<AccountId>>,
		AccountId: PartialEq<AccountId> + Decode + Clone,
	> EnsureOriginWithArg<O, DaoOrigin<AccountId>> for EnsureDao<AccountId>
{
	type Success = ();
	fn try_origin(o: O, dao_origin: &DaoOrigin<AccountId>) -> Result<Self::Success, O> {
		o.into().and_then(|o| match o {
			RawOrigin::Dao(ref dao_account_id) => {
				if dao_account_id == &dao_origin.dao_account_id {
					return Ok(())
				}

				Err(O::from(o))
			},
		})
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin(dao_origin: &DaoOrigin<AccountId>) -> Result<O, ()> {
		let dao_account_id = match dao_origin {
			ref origin => &origin.clone().dao_account_id,
		};
		Ok(O::from(RawOrigin::Dao(dao_account_id.clone())))
	}
}
