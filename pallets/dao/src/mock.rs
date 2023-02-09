use crate as pallet_dao;
use dao_primitives::{ApprovePropose, ApproveVote, ContainsDaoMember, InitializeDaoMembers};
use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	parameter_types,
	traits::{
		tokens::{
			fungibles::{
				metadata::{Inspect as MetadataInspect, Mutate as MetadataMutate},
				Create, Inspect, Mutate,
			},
			DepositConsequence, WithdrawConsequence,
		},
		ConstU16, ConstU32, ConstU64,
	},
	PalletId,
};
use frame_system as system;
use sp_core::{ConstU128, H256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, Extrinsic as ExtrinsicT, IdentityLookup},
};

use crate::crypto;
use frame_support::traits::fungibles::Transfer;
use serde_json::{json, Value};
use sp_core::sr25519::Signature;
use sp_runtime::{
	testing::{TestSignature, TestXt, UintAuthorityId},
	traits::{IdentifyAccount, Verify},
};
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
		DaoFactory: pallet_dao::{Pallet, Call, Storage, Event<T>},
	}
);

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

type Extrinsic = TestXt<RuntimeCall, ()>;

impl<C> frame_system::offchain::SendTransactionTypes<C> for Test
where
	RuntimeCall: From<C>,
{
	type OverarchingCall = RuntimeCall;
	type Extrinsic = Extrinsic;
}

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
	type AccountData = pallet_balances::AccountData<u64>;
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
	type Balance = u64;
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU64<1>;
	type AccountStore = System;
	type WeightInfo = ();
}

parameter_types! {
	pub const DaoPalletId: PalletId = PalletId(*b"py/sctld");
	pub static Members: HashMap<u32, Vec<AccountId>> = HashMap::new();
	pub static TechnicalCommittee: HashMap<u32, Vec<AccountId>> = HashMap::new();

	pub DaoName: String = "dao".into();
	pub DaoPurpose: String = "dao purpose".into();
	pub DaoMetadata: String = "dao metadata".into();

	pub TokenId: u32 = 0;
	pub TokenName: String = "dao_token".into();
	pub TokenSymbol: String = "sctl".into();
	pub TokenDecimals: u8 = 3;
	pub TokenInitialBalance: String = "1000000000".into();
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
	fn contains(dao_id: u32, who: &AccountId) -> Result<bool, DispatchError> {
		Ok(true)
	}
}

impl ApproveVote<u32, AccountId, H256> for TestCouncilProvider {
	fn approve_vote(dao_id: u32, hash: H256, approve: bool) -> Result<(), DispatchError> {
		Ok(())
	}
}

impl ApprovePropose<u32, AccountId, u128, H256> for TestCouncilProvider {
	fn approve_propose(
		dao_id: u32,
		threshold: u128,
		block_number: u32,
		hash: H256,
		approve: bool,
	) -> Result<(), DispatchError> {
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
	fn contains(dao_id: u32, who: &AccountId) -> Result<bool, DispatchError> {
		Ok(true)
	}
}

impl ApproveVote<u32, AccountId, H256> for TestTechnicalCommitteeProvider {
	fn approve_vote(dao_id: u32, hash: H256, approve: bool) -> Result<(), DispatchError> {
		Ok(())
	}
}

impl ApprovePropose<u32, AccountId, u128, H256> for TestTechnicalCommitteeProvider {
	fn approve_propose(
		dao_id: u32,
		threshold: u128,
		block_number: u32,
		hash: H256,
		approve: bool,
	) -> Result<(), DispatchError> {
		Ok(())
	}
}

pub struct TestAssetProvider;
impl Create<AccountId> for TestAssetProvider {
	fn create(
		_id: u32,
		_admin: AccountId,
		_is_sufficient: bool,
		_min_balance: u128,
	) -> DispatchResult {
		Ok(())
	}
}

impl Inspect<AccountId> for TestAssetProvider {
	type AssetId = u32;
	type Balance = u128;

	fn total_issuance(asset: Self::AssetId) -> Self::Balance {
		if asset == 2 {
			return 1
		}

		0
	}

	fn minimum_balance(asset: Self::AssetId) -> Self::Balance {
		1
	}

	fn balance(_asset: Self::AssetId, _who: &AccountId) -> Self::Balance {
		0
	}

	fn reducible_balance(
		_asset: Self::AssetId,
		_who: &AccountId,
		_keep_alive: bool,
	) -> Self::Balance {
		0
	}

	fn can_deposit(
		_asset: Self::AssetId,
		_who: &AccountId,
		_amount: Self::Balance,
		_mint: bool,
	) -> DepositConsequence {
		DepositConsequence::Success
	}

	fn can_withdraw(
		_asset: Self::AssetId,
		_who: &AccountId,
		_amount: Self::Balance,
	) -> WithdrawConsequence<Self::Balance> {
		WithdrawConsequence::Success
	}
}

impl Mutate<AccountId> for TestAssetProvider {
	fn mint_into(_asset: u32, _who: &AccountId, _amount: u128) -> DispatchResult {
		Ok(())
	}

	fn burn_from(_asset: u32, _who: &AccountId, _amount: u128) -> Result<u128, DispatchError> {
		Ok(0)
	}

	fn slash(_asset: u32, _who: &AccountId, _amount: u128) -> Result<u128, DispatchError> {
		Ok(0)
	}
}

impl MetadataInspect<AccountId> for TestAssetProvider {
	fn name(asset: u32) -> Vec<u8> {
		if asset == TokenId::get() {
			return TokenName::get().as_bytes().to_vec()
		}

		vec![]
	}

	fn symbol(asset: u32) -> Vec<u8> {
		if asset == TokenId::get() {
			return TokenSymbol::get().as_bytes().to_vec()
		}

		vec![]
	}

	fn decimals(asset: u32) -> u8 {
		if asset == TokenId::get() {
			return TokenDecimals::get()
		}

		0
	}
}

impl MetadataMutate<AccountId> for TestAssetProvider {
	fn set(
		_asset: u32,
		_from: &AccountId,
		_name: Vec<u8>,
		_symbol: Vec<u8>,
		_decimals: u8,
	) -> DispatchResult {
		Ok(())
	}
}

impl Transfer<AccountId> for TestAssetProvider {
	fn transfer(
		asset: u32,
		source: &AccountId,
		dest: &AccountId,
		amount: u128,
		keep_alive: bool,
	) -> Result<Self::Balance, DispatchError> {
		Ok(amount)
	}
}

impl pallet_dao::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type PalletId = DaoPalletId;
	type Currency = pallet_balances::Pallet<Test>;
	type DaoStringLimit = ConstU32<20>;
	type DaoMetadataLimit = ConstU32<20>;
	type AssetId = u32;
	type Balance = u128;
	type CouncilProvider = TestCouncilProvider;
	type AssetProvider = TestAssetProvider;
	type GovernanceApproveProvider = TestCouncilProvider;
	type AuthorityId = crypto::TestAuthId;
	type DaoMaxCouncilMembers = ConstU32<20>;
	type DaoMaxTechnicalCommitteeMembers = ConstU32<20>;
	type TechnicalCommitteeProvider = TestTechnicalCommitteeProvider;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	system::GenesisConfig::default().build_storage::<Test>().unwrap().into()
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
