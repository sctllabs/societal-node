use crate as pallet_dao_nft_governance;
use dao_primitives::{
	ContainsDaoMember, DaoOrigin, DaoPolicy, DaoProvider, DaoToken, DispatchResultWithDaoOrigin,
	InitializeDaoMembers, RawOrigin, RemoveDaoMembers,
};
use frame_support::{
	dispatch::DispatchError,
	parameter_types,
	traits::{
		AsEnsureOriginWithArg, ConstU16, ConstU32, ConstU64, EnsureOriginWithArg,
		EqualPrivilegeOnly,
	},
	PalletId,
};
use sp_core::{
	sr25519::{Public, Signature},
	ConstU128, H256,
};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
};
use sp_std::str::FromStr;

use crate::Config;
use frame_system::{EnsureRoot, EnsureSigned};
use serde_json::{json, Value};

use frame_support::{
	storage::KeyPrefixIterator,
	traits::tokens::nonfungibles_v2::{Inspect, InspectEnumerable},
};
use sp_runtime::traits::AccountIdConversion;
use std::collections::HashMap;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
		DaoNftGovernance: pallet_dao_nft_governance::{Pallet, Call, Storage, Event<T>},
		Preimage: pallet_preimage,
		Scheduler: pallet_scheduler::{Pallet, Call, Storage, Event<T>},
		Assets: pallet_dao_assets::{Pallet, Call, Storage, Config<T>, Event<T>},
	}
);

impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u128>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ConstU16<42>;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_balances::Config for Test {
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type Balance = u128;
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = System;
	type WeightInfo = ();
}

parameter_types! {
	pub const MaxProposals: u32 = 100;
	pub const MaxPendingProposals: u32 = 100;
	pub const MaxPendingVotes: u32 = 100;
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type Balance = u128;
	type Proposal = RuntimeCall;
	type ProposalMetadataLimit = ConstU32<750>;
	type MaxProposals = MaxProposals;
	type MaxVotes = ConstU32<100>;
	type DaoProvider = TestDaoProvider;
	type Preimages = Preimage;
	type NFTCollectionId = u32;
	type ItemId = u32;
	type NFTProvider = TestNFTProvider;
	type WeightInfo = ();
}

impl pallet_preimage::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type Currency = ();
	type ManagerOrigin = EnsureRoot<AccountId>;
	type BaseDeposit = ConstU32<0>;
	type ByteDeposit = ConstU32<0>;
}

impl pallet_scheduler::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type PalletsOrigin = OriginCaller;
	type RuntimeCall = RuntimeCall;
	type MaximumWeight = ();
	type ScheduleOrigin = EnsureRoot<AccountId>;
	type MaxScheduledPerBlock = ConstU32<100>;
	type WeightInfo = ();
	type OriginPrivilegeCmp = EqualPrivilegeOnly;
	type Preimages = ();
}

impl From<RawOrigin<Public>> for OriginCaller {
	fn from(_value: RawOrigin<Public>) -> Self {
		OriginCaller::system(frame_system::RawOrigin::Root)
	}
}

impl From<RawOrigin<Public>> for RuntimeOrigin {
	fn from(_value: RawOrigin<Public>) -> Self {
		RuntimeOrigin::from(frame_system::RawOrigin::Root)
	}
}

pub struct TestCouncilProvider;
impl InitializeDaoMembers<u32, AccountId> for TestCouncilProvider {
	fn initialize_members(
		dao_id: u32,
		source_members: Vec<AccountId>,
	) -> Result<(), DispatchError> {
		let mut members = HashMap::new();
		members.insert(dao_id, source_members.clone());

		Members::set(members);

		Ok(())
	}
}

impl ContainsDaoMember<u32, AccountId> for TestCouncilProvider {
	fn contains(_dao_id: u32, _who: &AccountId) -> Result<bool, DispatchError> {
		Ok(true)
	}
}

impl RemoveDaoMembers<u32> for TestCouncilProvider {
	fn remove_members(_dao_id: u32, _purge: bool) -> Result<(), DispatchError> {
		Ok(())
	}
}

pub struct TestTechnicalCommitteeProvider;
impl InitializeDaoMembers<u32, AccountId> for TestTechnicalCommitteeProvider {
	fn initialize_members(
		dao_id: u32,
		source_members: Vec<AccountId>,
	) -> Result<(), DispatchError> {
		let mut members = HashMap::new();
		members.insert(dao_id, source_members.clone());

		TechnicalCommittee::set(members);

		Ok(())
	}
}

impl ContainsDaoMember<u32, AccountId> for TestTechnicalCommitteeProvider {
	fn contains(_dao_id: u32, _who: &AccountId) -> Result<bool, DispatchError> {
		Ok(true)
	}
}

impl RemoveDaoMembers<u32> for TestTechnicalCommitteeProvider {
	fn remove_members(_dao_id: u32, _purge: bool) -> Result<(), DispatchError> {
		Ok(())
	}
}

impl pallet_dao_assets::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = u128;
	type RemoveItemsLimit = ConstU32<1000>;
	type AssetId = u128;
	type AssetIdParameter = codec::Compact<u128>;
	type Currency = Balances;
	type CreateOrigin = AsEnsureOriginWithArg<EnsureSigned<AccountId>>;
	type ForceOrigin = EnsureRoot<AccountId>;
	type AssetDeposit = ConstU128<0>;
	type AssetAccountDeposit = ConstU128<10>;
	type MetadataDepositBase = ConstU128<0>;
	type MetadataDepositPerByte = ConstU128<0>;
	type ApprovalDeposit = ConstU128<0>;
	type StringLimit = ConstU32<50>;
	type Freezer = Assets;
	type Extra = ();
	type CallbackHandle = ();
	type WeightInfo = pallet_dao_assets::weights::SubstrateWeight<Test>;
	type MaxLocks = ConstU32<10>;
}

parameter_types! {
	pub const DaoPalletId: PalletId = PalletId(*b"py/sctld");
	pub static Members: HashMap<u32, Vec<AccountId>> = HashMap::new();
	pub static TechnicalCommittee: HashMap<u32, Vec<AccountId>> = HashMap::new();

	pub DaoName: String = "dao".into();
	pub DaoPurpose: String = "dao purpose".into();
	pub DaoMetadata: String = "dao metadata".into();

	pub TokenId: u128 = 0;
	pub TokenName: String = "dao_token".into();
	pub TokenSymbol: String = "sctl".into();
	pub TokenDecimals: u8 = 3;
	pub TokenInitialBalance: String = "1000000000".into();
}

pub struct TestDaoProvider;
impl DaoProvider<AccountId, H256> for TestDaoProvider {
	type Id = u32;
	type AssetId = u128;
	type Policy = DaoPolicy;
	type Origin = RuntimeOrigin;
	type ApproveOrigin = AsEnsureOriginWithArg<frame_system::EnsureRoot<AccountId>>;
	type NFTCollectionId = u32;

	fn dao_account_id(id: Self::Id) -> AccountId {
		PalletId(*b"py/sctld").into_sub_account_truncating(id)
	}

	fn dao_token(_id: Self::Id) -> Result<DaoToken<Self::AssetId, Vec<u8>>, DispatchError> {
		Ok(DaoToken::FungibleToken(0))
	}

	fn policy(_id: Self::Id) -> Result<Self::Policy, DispatchError> {
		Ok(DaoPolicy {
			proposal_period: 3,
			approve_origin: Default::default(),
			governance: None,
			bounty_payout_delay: Default::default(),
			bounty_update_period: Default::default(),
			spend_period: Default::default(),
		})
	}

	fn dao_nft_collection_id(
		_id: Self::Id,
	) -> Result<Option<Self::NFTCollectionId>, DispatchError> {
		Ok(Some(0))
	}

	fn ensure_approved(
		origin: Self::Origin,
		dao_id: Self::Id,
	) -> DispatchResultWithDaoOrigin<AccountId> {
		let dao_account_id = Self::dao_account_id(dao_id);
		let approve_origin = Self::policy(dao_id)?.approve_origin;
		let dao_origin = DaoOrigin { dao_account_id, proportion: approve_origin };

		Self::ApproveOrigin::ensure_origin(origin, &dao_origin)?;

		Ok(dao_origin)
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn create_dao(
		_founder: AccountId,
		_council: Vec<AccountId>,
		_technical_committee: Vec<AccountId>,
		_data: Vec<u8>,
	) -> Result<(), DispatchError> {
		Ok(())
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn approve_dao(_dao_hash: H256, _approve: bool) -> Result<(), DispatchError> {
		Ok(())
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin(dao_origin: &DaoOrigin<AccountId>) -> Result<Self::Origin, ()> {
		Self::ApproveOrigin::try_successful_origin(dao_origin)
	}
}

pub struct TestNFTProvider;
impl Inspect<AccountId> for TestNFTProvider {
	type ItemId = u32;
	type CollectionId = u32;

	fn owner(_collection: &Self::CollectionId, _item: &Self::ItemId) -> Option<AccountId> {
		Public::from_str("5CNJv1vQjABY9W3BtsV2tzaLCjZepWXaYYzuDGWUUNVvMjcG").ok()
	}
}

impl InspectEnumerable<AccountId> for TestNFTProvider {
	type CollectionsIterator = KeyPrefixIterator<u32>;
	type ItemsIterator = KeyPrefixIterator<u32>;
	type OwnedIterator = KeyPrefixIterator<(u32, u32)>;
	type OwnedInCollectionIterator = KeyPrefixIterator<u32>;

	fn collections() -> Self::CollectionsIterator {
		KeyPrefixIterator::new(vec![], vec![], |_| Ok(0))
	}

	fn items(_collection: &Self::CollectionId) -> Self::ItemsIterator {
		KeyPrefixIterator::new(vec![], vec![], |_| Ok(0))
	}

	fn owned(_who: &AccountId) -> Self::OwnedIterator {
		KeyPrefixIterator::new(vec![], vec![], |_| Ok((0, 0)))
	}

	fn owned_in_collection(
		_collection: &Self::CollectionId,
		_who: &AccountId,
	) -> Self::OwnedInCollectionIterator {
		KeyPrefixIterator::new(vec![], vec![], |_| Ok(0))
	}
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	frame_system::GenesisConfig::default().build_storage::<Test>().unwrap().into()
}

pub fn get_dao_json() -> Value {
	json!({
		"name": DaoName::get(),
		"purpose": DaoPurpose::get(),
		"metadata": DaoMetadata::get(),
		"policy": {
			"proposal_period": 100
		},
		"token": {
			"token_id": TokenId::get(),
			"initial_balance": TokenInitialBalance::get(),
			"metadata": {
				"name": TokenName::get(),
				"symbol": TokenSymbol::get(),
				"decimals": TokenDecimals::get()
			}
		}
	})
}
