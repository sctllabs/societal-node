use crate as pallet_dao_eth_governance;
use dao_primitives::{ContainsDaoMember, InitializeDaoMembers, RawOrigin};
use eth_primitives::EthService;
use frame_support::{
	dispatch::DispatchError,
	parameter_types,
	traits::{AsEnsureOriginWithArg, ConstU16, ConstU32, ConstU64, EqualPrivilegeOnly},
	PalletId,
};
use sp_core::{
	sr25519::{Public, Signature},
	ConstU128, H256,
};
use sp_runtime::{
	testing::{Header, TestXt},
	traits::{BlakeTwo256, Extrinsic as ExtrinsicT, IdentifyAccount, IdentityLookup, Verify},
};

use crate::Config;
use frame_system::{EnsureRoot, EnsureSigned};
use pallet_dao::crypto;
use serde_json::{json, Value};

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
		DaoEthGovernance: pallet_dao_eth_governance::{Pallet, Call, Storage, Event<T>},
		DaoFactory: pallet_dao::{Pallet, Call, Storage, Event<T>, Config<T>},
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
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type Balance = u128;
	type Proposal = RuntimeCall;
	type ProposalMetadataLimit = ConstU32<750>;
	type EthRpcUrlLimit = ();
	type MaxProposals = MaxProposals;
	type MaxPendingProposals = MaxPendingProposals;
	type MaxPendingVotes = MaxPendingVotes;
	type MaxVotes = ConstU32<100>;
	type DaoProvider = DaoFactory;
	type Preimages = Preimage;
	type AuthorityId = crypto::TestAuthId;
	type OffchainEthService = EthService<DaoEthGovernance>;
	type Scheduler = Scheduler;
	type PalletsOrigin = OriginCaller;
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

type Extrinsic = TestXt<RuntimeCall, ()>;

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Test
where
	RuntimeCall: From<LocalCall>,
{
	fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
		call: RuntimeCall,
		_public: <Signature as Verify>::Signer,
		_account: AccountId,
		nonce: u64,
	) -> Option<(RuntimeCall, <Extrinsic as ExtrinsicT>::SignaturePayload)> {
		Some((call, (nonce, ())))
	}
}

impl frame_system::offchain::SigningTypes for Test {
	type Public = <Signature as Verify>::Signer;
	type Signature = Signature;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Test
where
	RuntimeCall: From<C>,
{
	type OverarchingCall = RuntimeCall;
	type Extrinsic = Extrinsic;
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

impl pallet_dao::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type PalletId = DaoPalletId;
	type Currency = pallet_balances::Pallet<Test>;
	type DaoNameLimit = ConstU32<20>;
	type DaoStringLimit = ConstU32<100>;
	type DaoMetadataLimit = ConstU32<750>;
	type AssetId = u128;
	type Balance = u128;
	type CouncilProvider = TestCouncilProvider;
	type AssetProvider = Assets;
	type AuthorityId = crypto::TestAuthId;
	type DaoMaxCouncilMembers = ConstU32<100>;
	type DaoMaxTechnicalCommitteeMembers = ConstU32<100>;
	type DaoMaxPendingItems = ConstU32<100>;
	type TechnicalCommitteeProvider = TestTechnicalCommitteeProvider;
	type OffchainEthService = ();
	type RuntimeCall = RuntimeCall;
	type DaoMinTreasurySpendPeriod = ConstU32<20>;
	type ApproveOrigin = AsEnsureOriginWithArg<EnsureRoot<AccountId>>;
	type Scheduler = Scheduler;
	type PalletsOrigin = OriginCaller;
	type Preimages = ();
	type SpendDaoFunds = ();
	type DaoReferendumScheduler = ();
	type WeightInfo = ();

	#[cfg(feature = "runtime-benchmarks")]
	type DaoReferendumBenchmarkHelper = ();
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

	pub TokenAddress: String = "0x439ACbC2FAE8E4b9115e702AeBeAa9977621017C".to_string();
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
		"token_address": TokenAddress::get()
	})
}
