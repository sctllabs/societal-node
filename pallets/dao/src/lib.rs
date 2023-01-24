#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, HasCompact};
use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	pallet_prelude::*,
	traits::{
		tokens::fungibles::{metadata::Mutate as MetadataMutate, Create, Inspect, Mutate},
		Currency, EnsureOriginWithArg, Get, ReservableCurrency,
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

type PendingDaoOf<T> = PendingDao<
	<T as frame_system::Config>::AccountId,
	<T as Config>::AssetId,
	BoundedVec<u8, <T as Config>::DaoStringLimit>,
	BoundedVec<u8, <T as Config>::DaoStringLimit>,
	BoundedVec<<T as frame_system::Config>::AccountId, <T as Config>::DaoMaxCouncilMembers>,
	BoundedVec<
		<T as frame_system::Config>::AccountId,
		<T as Config>::DaoMaxTechnicalCommitteeMembers,
	>,
>;

/// Dao ID. Just a `u32`.
pub type DaoId = u32;

type AssetId<T> = <T as Config>::AssetId;
type Balance<T> = <T as Config>::Balance;

pub type BlockNumber = u32;

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"sctl");

const UNSIGNED_TXS_PRIORITY: u64 = 100;

const ERC20_TOKEN_TOTAL_SUPPLY_SIGNATURE: &str =
	"0x18160ddd0000000000000000000000000000000000000000000000000000000000000000";
const ERC20_TOKEN_BALANCE_OF_SIGNATURE_PREFIX: &str = "0x70a08231000000000000000000000000";

const FETCH_TIMEOUT_PERIOD: u64 = 3000; // in milli-seconds
const LOCK_TIMEOUT_EXPIRATION: u64 = FETCH_TIMEOUT_PERIOD + 1000; // in milli-seconds
const LOCK_BLOCK_EXPIRATION: u32 = 3; // in block number

const ONCHAIN_TX_KEY: &[u8] = b"societal-dao::storage::tx";

const TOKEN_MIN_BALANCE: u128 = 1;

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
pub struct EthRPCResponse {
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub result: Vec<u8>,
}

#[derive(Debug, Encode, Decode, Clone, Default, Deserialize, PartialEq, Eq)]
pub enum OffchainData<AccountId, Hash> {
	#[default]
	Default,
	ApproveDao {
		dao_hash: Hash,
		token_address: Vec<u8>,
	},
	ApproveProposal {
		dao_id: u32,
		token_address: Vec<u8>,
		who: AccountId,
		threshold: u32,
		hash: Hash,
		length_bound: u32,
	},
	ApproveTreasuryProposal {
		dao_id: u32,
		token_address: Vec<u8>,
		who: AccountId,
		hash: Hash,
	},
	ApproveVote {
		dao_id: u32,
		token_address: Vec<u8>,
		who: AccountId,
		hash: Hash,
	},
}

#[derive(Debug, Deserialize, Encode, Decode, Default)]
struct IndexingData<AccountId, Hash>(Vec<u8>, OffchainData<AccountId, Hash>);

#[frame_support::pallet]
pub mod pallet {
	pub use super::*;
	use frame_support::traits::{fungibles::Transfer, EnsureOriginWithArg};
	use frame_system::{
		offchain::{AppCrypto, CreateSignedTransaction, SubmitTransaction},
		pallet_prelude::*,
	};
	use serde_json::json;
	use sp_runtime::{
		offchain::{
			storage::StorageValueRef,
			storage_lock::{BlockAndTime, StorageLock},
		},
		traits::{Hash, Zero},
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
		/// The outer origin type.
		type RuntimeOrigin: From<RawOrigin<Self::AccountId>>;

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
		type DaoMaxCouncilMembers: Get<u32>;

		#[pallet::constant]
		type DaoMaxTechnicalCommitteeMembers: Get<u32>;

		#[pallet::constant]
		type DaoTokenMinBalanceLimit: Get<u128>;

		#[pallet::constant]
		type DaoTokenVotingMinThreshold: Get<u128>;

		type CouncilProvider: InitializeDaoMembers<u32, Self::AccountId>
			+ ContainsDaoMember<u32, Self::AccountId>;

		type CouncilApproveProvider: ApprovePropose<u32, Self::AccountId, Self::Hash>
			+ ApproveVote<u32, Self::AccountId, Self::Hash>;

		type TechnicalCommitteeProvider: InitializeDaoMembers<u32, Self::AccountId>
			+ ContainsDaoMember<u32, Self::AccountId>;

		type ApproveTreasuryPropose: ApproveTreasuryPropose<u32, Self::AccountId, Self::Hash>;

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

		type ApproveOrigin: EnsureOriginWithArg<
			<Self as frame_system::Config>::RuntimeOrigin,
			DaoOrigin<Self::AccountId>,
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

	/// The ideal number of staking participants.
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
			log::info!("Entering off-chain worker");

			let key = Self::derived_key(block_number);
			let storage_ref = StorageValueRef::persistent(&key);

			if let Ok(Some(data)) = storage_ref.get::<IndexingData<T::AccountId, T::Hash>>() {
				log::info!(
					"off-chain indexing data: {:?}, {:?}",
					str::from_utf8(&data.0).unwrap_or("error"),
					data.1
				);

				let offchain_data = data.1;

				let mut call = None;
				match offchain_data.clone() {
					OffchainData::ApproveDao { dao_hash, token_address } =>
						match Self::parse_token_balance(Self::fetch_token_total_supply(
							token_address,
						)) {
							Ok(total_supply) => {
								call =
									Some(Call::approve_dao { dao_hash, approve: total_supply > 0 });
							},
							Err(e) => {
								log::error!("offchain_worker error: {:?}", e);

								call = Some(Call::approve_dao { dao_hash, approve: false });
							},
						},
					OffchainData::ApproveProposal {
						dao_id,
						token_address,
						who,
						threshold,
						hash,
						length_bound,
					} => {
						match Self::parse_token_balance(Self::fetch_token_balance_of(
							token_address,
							who,
						)) {
							Ok(token_balance) => {
								call = Some(Call::approve_council_propose {
									dao_id,
									hash,
									approve: token_balance >= T::DaoTokenVotingMinThreshold::get(),
								});
							},
							Err(e) => {
								log::error!("offchain_worker error: {:?}", e);

								call = Some(Call::approve_council_propose {
									dao_id,
									hash,
									approve: false,
								});
							},
						}
					},
					OffchainData::ApproveTreasuryProposal { dao_id, token_address, who, hash } =>
						match Self::parse_token_balance(Self::fetch_token_balance_of(
							token_address,
							who,
						)) {
							Ok(token_balance) => {
								call = Some(Call::approve_treasury_propose {
									dao_id,
									hash,
									approve: token_balance >= T::DaoTokenVotingMinThreshold::get(),
								});
							},
							Err(e) => {
								log::error!("offchain_worker error: {:?}", e);

								call = Some(Call::approve_treasury_propose {
									dao_id,
									hash,
									approve: false,
								});
							},
						},
					OffchainData::ApproveVote { dao_id, token_address, who, hash } =>
						match Self::parse_token_balance(Self::fetch_token_balance_of(
							token_address,
							who,
						)) {
							Ok(token_balance) => {
								call = Some(Call::approve_proposal_vote {
									dao_id,
									hash,
									approve: token_balance >= T::DaoTokenVotingMinThreshold::get(),
								});
							},
							Err(e) => {
								log::error!("offchain_worker error: {:?}", e);

								call = Some(Call::approve_treasury_propose {
									dao_id,
									hash,
									approve: false,
								});
							},
						},
					_ => {},
				};

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
					match offchain_data {
						OffchainData::ApproveDao { dao_hash, .. } => {
							log::error!("dao approval error: {:?}", dao_hash);
						},
						OffchainData::ApproveProposal { dao_id, hash, .. } => {
							log::error!(
								"proposal approval error: dao_id: {:?}, proposal_hash: {:?}",
								dao_id,
								hash
							);
						},
						OffchainData::ApproveTreasuryProposal { dao_id, hash, .. } => {
							log::error!(
								"treasury proposal approval error: dao_id: {:?}, proposal_hash: {:?}",
								dao_id,
								hash
							);
						},
						_ => {},
					}
				}

				return
			}

			log::info!("no off-chain indexing data retrieved.");
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
				Call::approve_dao { dao_hash, approve } => valid_tx(b"approve_dao".to_vec()),
				Call::approve_council_propose { dao_id, hash, approve } =>
					valid_tx(b"approve_council_dao".to_vec()),
				Call::approve_treasury_propose { dao_id, hash, approve } =>
					valid_tx(b"approve_treasury_propose".to_vec()),
				Call::approve_proposal_vote { dao_id, hash, approve } =>
					valid_tx(b"approve_proposal_vote".to_vec()),
				_ => InvalidTransaction::Call.into(),
			}
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		DaoRegistered(u32, T::AccountId),
		DaoPendingApproval(u32, T::AccountId),
		DaoTokenTransferred {
			dao_id: DaoId,
			token_id: T::AssetId,
			beneficiary: T::AccountId,
			amount: Balance<T>,
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
		TokenBalanceInvalid,
		TokenBalanceLow,
		TokenTransferFailed,
		TokenAddressInvalid,
		InvalidInput,
		PolicyNotExist,
		// Error returned when fetching http
		HttpFetchingError,
		OffchainUnsignedTxError,
		RpcUrlTooLong,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		// TODO: calculate dynamic weight
		#[pallet::weight(10_000 + T::DbWeight::get().writes(4).ref_time())]
		pub fn create_dao(
			origin: OriginFor<T>,
			council: Vec<<T::Lookup as StaticLookup>::Source>,
			technical_committee: Vec<<T::Lookup as StaticLookup>::Source>,
			data: Vec<u8>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			let DaoPayload { name, purpose, metadata, token, token_id, token_address, policy } =
				serde_json::from_slice::<DaoPayload>(&data)
					.map_err(|_| Error::<T>::InvalidInput)?;

			let dao_id = <NextDaoId<T>>::get();
			let dao_account_id = Self::account_id(dao_id);

			let dao_name = BoundedVec::<u8, T::DaoStringLimit>::try_from(name)
				.map_err(|_| Error::<T>::NameTooLong)?;

			let dao_purpose = BoundedVec::<u8, T::DaoStringLimit>::try_from(purpose)
				.map_err(|_| Error::<T>::PurposeTooLong)?;

			let dao_metadata = BoundedVec::<u8, T::DaoStringLimit>::try_from(metadata)
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

			let mut technical_committee_members: Vec<T::AccountId> = vec![];
			for member in technical_committee.into_iter() {
				let account = T::Lookup::lookup(member)?;

				technical_committee_members.push(account);
			}

			let founder = who.clone();
			let config = DaoConfig { name: dao_name, purpose: dao_purpose, metadata: dao_metadata };

			let mut has_token_id: Option<AssetId<T>> = None;

			if let Some(token) = token {
				let token_id = Self::u32_to_asset_id(token.token_id);
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
						false,
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
					.map_err(|_| Error::<T>::TokenCreateFailed)?;

					if token_initial_balance.gt(&Self::u128_to_balance(0)) {
						T::AssetProvider::mint_into(
							token_id,
							&dao_account_id,
							token_initial_balance,
						)
						.map_err(|_| Error::<T>::TokenCreateFailed)?;
					}
				}
			}

			if has_token_id.is_none() {
				if let Some(id) = token_id {
					let token_id = Self::u32_to_asset_id(id);
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
					let data: IndexingData<T::AccountId, T::Hash> = IndexingData(
						b"approve_dao".to_vec(),
						OffchainData::ApproveDao { dao_hash, token_address },
					);

					offchain_index::set(&key, &data.encode());

					let council = BoundedVec::<T::AccountId, T::DaoMaxCouncilMembers>::try_from(
						council_members.clone(),
					)
					.map_err(|_| Error::<T>::CouncilMembersOverflow)?;

					let technical_committee = BoundedVec::<
						T::AccountId,
						T::DaoMaxTechnicalCommitteeMembers,
					>::try_from(technical_committee_members.clone())
					.map_err(|_| Error::<T>::CouncilMembersOverflow)?;

					PendingDaos::<T>::insert(
						dao_hash,
						PendingDao { dao, policy, council, technical_committee },
					);

					Self::deposit_event(Event::DaoPendingApproval(dao_id, who));

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

			Self::do_register_dao(dao, policy, council_members, technical_committee_members)
		}

		#[pallet::weight(10_000)]
		pub fn transfer_token(
			origin: OriginFor<T>,
			dao_id: DaoId,
			#[pallet::compact] amount: Balance<T>,
			beneficiary: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResult {
			let dao_account_id = Self::dao_account_id(dao_id);
			let approve_origin = Self::policy(dao_id)?.approve_origin;
			T::ApproveOrigin::ensure_origin(
				origin,
				&DaoOrigin { dao_account_id: dao_account_id.clone(), proportion: approve_origin },
			)?;

			let dao_token = Self::dao_token(dao_id)?;

			let beneficiary = T::Lookup::lookup(beneficiary)?;

			match dao_token {
				DaoToken::FungibleToken(token_id) => {
					T::AssetProvider::transfer(
						token_id,
						&dao_account_id,
						&beneficiary,
						amount,
						true,
					)?;

					Self::deposit_event(Event::DaoTokenTransferred {
						dao_id,
						token_id,
						beneficiary,
						amount,
					});
				},
				DaoToken::EthTokenAddress(_) => {
					// TODO: handle token_address
				},
			}

			Ok(())
		}

		#[pallet::weight(10_000)]
		pub fn approve_dao(
			origin: OriginFor<T>,
			dao_hash: T::Hash,
			approve: bool,
		) -> DispatchResult {
			ensure_none(origin)?;

			let PendingDao { dao, policy, council, technical_committee } =
				match <PendingDaos<T>>::take(dao_hash) {
					None => return Err(Error::<T>::DaoNotExist.into()),
					Some(dao) => dao,
				};

			if approve {
				return Self::do_register_dao(
					dao,
					policy,
					council.to_vec(),
					technical_committee.to_vec(),
				)
			}

			Ok(())
		}

		#[pallet::weight(10_000)]
		pub fn approve_council_propose(
			origin: OriginFor<T>,
			dao_id: u32,
			hash: T::Hash,
			approve: bool,
		) -> DispatchResult {
			ensure_none(origin)?;

			T::CouncilApproveProvider::approve_propose(dao_id, hash, approve)?;

			Ok(())
		}

		#[pallet::weight(10_000)]
		pub fn approve_treasury_propose(
			origin: OriginFor<T>,
			dao_id: u32,
			hash: T::Hash,
			approve: bool,
		) -> DispatchResult {
			ensure_none(origin)?;

			T::ApproveTreasuryPropose::approve_treasury_propose(dao_id, hash, approve)?;

			Ok(())
		}

		#[pallet::weight(10_000)]
		pub fn approve_proposal_vote(
			origin: OriginFor<T>,
			dao_id: u32,
			hash: T::Hash,
			approve: bool,
		) -> DispatchResult {
			ensure_none(origin)?;

			T::CouncilApproveProvider::approve_vote(dao_id, hash, approve)?;

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

		pub fn do_register_dao(
			mut dao: DaoOf<T>,
			policy: PolicyOf,
			council: Vec<T::AccountId>,
			technical_committee: Vec<T::AccountId>,
		) -> DispatchResult {
			let dao_id = <NextDaoId<T>>::get();

			dao.account_id = Self::account_id(dao_id);
			dao.status = DaoStatus::Success;

			let founder = dao.clone().founder;

			Policies::<T>::insert(dao_id, policy);

			Daos::<T>::insert(dao_id, dao);

			T::CouncilProvider::initialize_members(dao_id, council)?;

			T::TechnicalCommitteeProvider::initialize_members(dao_id, technical_committee)?;

			<NextDaoId<T>>::put(dao_id.checked_add(1).unwrap());

			Self::deposit_event(Event::DaoRegistered(dao_id, founder));

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

		// TODO: re-work conversion
		fn parse_token_balance(
			response: Result<EthRPCResponse, Error<T>>,
		) -> Result<u128, Error<T>> {
			let result = response?.result;

			let value = Result::unwrap_or(str::from_utf8(&result), "");
			let mut value_stripped = value;
			if value.starts_with("0x") {
				value_stripped = match value.strip_prefix("0x") {
					None => "",
					Some(stripped) => stripped,
				}
			}

			let token_balance = Result::unwrap_or(u128::from_str_radix(value_stripped, 16), 0_u128);

			Ok(token_balance)
		}

		// TODO: add balanceOf at some block
		fn fetch_token_balance_of(
			token_address: Vec<u8>,
			who: T::AccountId,
		) -> Result<EthRPCResponse, Error<T>> {
			let data =
				[ERC20_TOKEN_BALANCE_OF_SIGNATURE_PREFIX, &format!("{:?}", who)[..40]].concat();

			Self::fetch_from_eth(token_address, &data[..])
		}

		fn fetch_token_total_supply(token_address: Vec<u8>) -> Result<EthRPCResponse, Error<T>> {
			Self::fetch_from_eth(token_address, ERC20_TOKEN_TOTAL_SUPPLY_SIGNATURE)
		}

		fn fetch_from_eth(token_address: Vec<u8>, data: &str) -> Result<EthRPCResponse, Error<T>> {
			let key_suffix = &token_address[..];
			let key =
				[&b"societal-dao::eth::"[..19], key_suffix, &b"::"[..2], data.as_bytes()].concat();
			let s_info = StorageValueRef::persistent(&key[..]);

			if let Ok(Some(eth_rpc_response)) = s_info.get::<EthRPCResponse>() {
				return Ok(eth_rpc_response)
			}

			let lock_key =
				[&b"societal-dao::eth::lock::"[..25], key_suffix, &b"::"[..2], data.as_bytes()]
					.concat();
			let mut lock = StorageLock::<BlockAndTime<Self>>::with_block_and_time_deadline(
				&lock_key[..],
				LOCK_BLOCK_EXPIRATION,
				offchain::Duration::from_millis(LOCK_TIMEOUT_EXPIRATION),
			);

			if let Ok(_guard) = lock.try_lock() {
				let to =
					str::from_utf8(&token_address[..]).expect("Failed to convert token address");
				let json = json!({
					"jsonrpc": "2.0",
					"method": "eth_call",
					"id": 1,
					"params": [
						{
							"to": to,
							"data": data
						},
						"latest"
					]
				});
				let body = &serde_json::to_vec(&json).expect("Failed to serialize")[..];

				match Self::fetch_n_parse(vec![body]) {
					Ok(eth_rpc_response) => {
						s_info.set(&eth_rpc_response);

						return Ok(eth_rpc_response)
					},
					Err(err) => return Err(err),
				}
			}

			Err(<Error<T>>::HttpFetchingError)
		}

		fn fetch_n_parse(body: Vec<&[u8]>) -> Result<EthRPCResponse, Error<T>> {
			let resp_bytes = Self::fetch_from_remote(body).map_err(|e| {
				log::error!("fetch_from_remote error: {:?}", e);
				<Error<T>>::HttpFetchingError
			})?;

			let resp_str =
				str::from_utf8(&resp_bytes).map_err(|_| <Error<T>>::HttpFetchingError)?;

			let eth_rpc_response: EthRPCResponse =
				serde_json::from_str(resp_str).map_err(|_| <Error<T>>::HttpFetchingError)?;

			Ok(eth_rpc_response)
		}

		fn fetch_from_remote(body: Vec<&[u8]>) -> Result<Vec<u8>, Error<T>> {
			let eth_rpc_url = &EthRpcUrl::<T>::get().to_vec()[..];
			let request = offchain::http::Request::post(str::from_utf8(eth_rpc_url).unwrap(), body);

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

impl<T: Config> DaoProvider<T::Hash> for Pallet<T> {
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

	fn ensure_treasury_proposal_allowed(
		id: Self::Id,
		who: &Self::AccountId,
		hash: T::Hash,
		force: bool,
	) -> Result<AccountTokenBalance, DispatchError> {
		let token_balance = Self::ensure_token_balance(id, who, force)?;

		if let AccountTokenBalance::Offchain { token_address } = token_balance.clone() {
			let key = Self::derived_key(frame_system::Pallet::<T>::block_number());

			let data: IndexingData<T::AccountId, T::Hash> = IndexingData(
				b"approve_treasury_propose".to_vec(),
				OffchainData::ApproveTreasuryProposal {
					dao_id: id,
					token_address,
					who: who.clone(),
					hash,
				},
			);

			offchain_index::set(&key, &data.encode());
		}

		Ok(token_balance)
	}

	fn ensure_proposal_allowed(
		id: Self::Id,
		who: &Self::AccountId,
		threshold: u32,
		hash: T::Hash,
		length_bound: u32,
		force: bool,
	) -> Result<AccountTokenBalance, DispatchError> {
		let token_balance = Self::ensure_token_balance(id, who, force)?;

		if let AccountTokenBalance::Offchain { token_address } = token_balance.clone() {
			let key = Self::derived_key(frame_system::Pallet::<T>::block_number());

			let data: IndexingData<T::AccountId, T::Hash> = IndexingData(
				b"approve_council_propose".to_vec(),
				OffchainData::ApproveProposal {
					dao_id: id,
					token_address,
					who: who.clone(),
					threshold,
					hash,
					length_bound,
				},
			);

			offchain_index::set(&key, &data.encode());
		}

		Ok(token_balance)
	}

	fn ensure_voting_allowed(
		id: Self::Id,
		who: &Self::AccountId,
		hash: T::Hash,
		force: bool,
	) -> Result<AccountTokenBalance, DispatchError> {
		let token_balance = Self::ensure_token_balance(id, who, force)?;

		if let AccountTokenBalance::Offchain { token_address } = token_balance.clone() {
			let key = Self::derived_key(frame_system::Pallet::<T>::block_number());

			let data: IndexingData<T::AccountId, T::Hash> = IndexingData(
				b"approve_proposal_vote".to_vec(),
				OffchainData::ApproveVote { dao_id: id, token_address, who: who.clone(), hash },
			);

			offchain_index::set(&key, &data.encode());
		}

		Ok(token_balance)
	}

	fn ensure_token_balance(
		id: Self::Id,
		who: &Self::AccountId,
		force: bool,
	) -> Result<AccountTokenBalance, DispatchError> {
		let dao = Daos::<T>::get(id).ok_or(Error::<T>::DaoNotExist)?;

		match dao.token {
			DaoToken::FungibleToken(token_id) => {
				if force &&
					T::AssetProvider::balance(token_id, who) <
						Self::u128_to_balance(T::DaoTokenVotingMinThreshold::get())
				{
					return Err(Error::<T>::TokenBalanceLow.into())
				}

				Ok(AccountTokenBalance::Sufficient)
			},
			DaoToken::EthTokenAddress(token_address) =>
				Ok(AccountTokenBalance::Offchain { token_address: token_address.to_vec() }),
		}
	}

	fn dao_account_id(id: Self::Id) -> Self::AccountId {
		Self::account_id(id)
	}

	fn dao_token(id: Self::Id) -> Result<DaoToken<Self::AssetId, Vec<u8>>, DispatchError> {
		match Daos::<T>::get(&id) {
			None => Err(Error::<T>::DaoNotExist.into()),
			Some(dao) => Ok(match dao.token {
				DaoToken::FungibleToken(token_id) => DaoToken::FungibleToken(token_id),
				DaoToken::EthTokenAddress(token_address) =>
					DaoToken::EthTokenAddress(token_address.to_vec()),
			}),
		}
	}
}

impl<T: Config> BlockNumberProvider for Pallet<T> {
	type BlockNumber = T::BlockNumber;
	fn current_block_number() -> Self::BlockNumber {
		<frame_system::Pallet<T>>::block_number()
	}
}

pub struct EnsureDao<AccountId>(PhantomData<AccountId>);
impl<
		O: Into<Result<RawOrigin<AccountId>, O>> + From<RawOrigin<AccountId>>,
		AccountId: PartialEq<AccountId> + Decode,
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
			r => Err(O::from(r)),
		})
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin(dao_origin: &DaoOrigin<AccountId>) -> Result<O, ()> {
		let zero_account_id =
			AccountId::decode(&mut sp_runtime::traits::TrailingZeroInput::zeroes())
				.expect("infinite length input; no invalid inputs for type; qed");
		Ok(O::from(RawOrigin::Dao(zero_account_id)))
	}
}
