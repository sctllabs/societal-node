use crate as pallet_dao;
use dao_primitives::{
	ApprovePropose, ApproveTreasuryPropose, ApproveVote, ContainsDaoMember, InitializeDaoMembers,
};
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
	traits::{BlakeTwo256, IdentityLookup, Extrinsic as ExtrinsicT},
};

use frame_support::traits::fungibles::Transfer;
use serde_json::{json, Value};
use sp_core::sr25519::Signature;
use sp_runtime::{testing::TestXt, traits::Verify};
use std::collections::HashMap;
use sp_runtime::traits::IdentifyAccount;
use crate::crypto;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

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

// impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Test
// where
// 	RuntimeCall: From<LocalCall>,
// {
// 	fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
// 		call: RuntimeCall,
// 		_public: <Signature as Verify>::Signer,
// 		_account: u64,
// 		nonce: u64,
// 	) -> Option<(
// 		RuntimeCall,
// 		<UncheckedExtrinsic as sp_runtime::traits::Extrinsic>::SignaturePayload,
// 	)> {
// 		Some((call, (nonce, ())))
// 	}
// }

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Test
	where
		RuntimeCall: From<LocalCall>,
{
	fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
		call: RuntimeCall,
		_public: <Signature as Verify>::Signer,
		_account: u64,
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
	// type AccountId = u64;
	type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
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
	pub static Members: HashMap<u32, Vec<u64>> = HashMap::new();

	pub DaoName: String = "dao".into();
	pub DaoPurpose: String = "dao purpose".into();
	pub DaoMetadata: String = "dao metadata".into();

	pub TokenId: u32 = 0;
	pub TokenName: String = "dao_token".into();
	pub TokenSymbol: String = "sctl".into();
	pub TokenDecimals: u8 = 3;
	pub TokenMinBalance: String = "100000000000".into();
}

pub struct TestCouncilProvider;
impl InitializeDaoMembers<u32, u64> for TestCouncilProvider {
	fn initialize_members(dao_id: u32, source_members: Vec<u64>) -> Result<(), DispatchError> {
		let mut members = HashMap::new();
		members.insert(dao_id, source_members.clone());

		Members::set(members);

		Ok(())
	}
}

impl ContainsDaoMember<u32, u64> for TestCouncilProvider {
	fn contains(dao_id: u32, who: &u64) -> Result<bool, DispatchError> {
		Ok(true)
	}
}

impl ApproveVote<u32, u64, H256> for TestCouncilProvider {
	fn approve_vote(dao_id: u32, hash: H256, approve: bool) -> Result<(), DispatchError> {
		Ok(())
	}
}

impl ApprovePropose<u32, u64, H256> for TestCouncilProvider {
	fn approve_propose(dao_id: u32, hash: H256, approve: bool) -> Result<(), DispatchError> {
		Ok(())
	}
}

pub struct TestAssetProvider;
impl Create<u64> for TestAssetProvider {
	fn create(_id: u32, _admin: u64, _is_sufficient: bool, _min_balance: u128) -> DispatchResult {
		Ok(())
	}
}

impl Inspect<u64> for TestAssetProvider {
	type AssetId = u32;
	type Balance = u128;

	fn total_issuance(asset: Self::AssetId) -> Self::Balance {
		if asset == 2 {
			return 1
		}

		0
	}

	fn minimum_balance(asset: Self::AssetId) -> Self::Balance {
		if asset == TokenId::get() {
			return TokenMinBalance::get().parse::<u128>().unwrap()
		}

		0
	}

	fn balance(_asset: Self::AssetId, _who: &u64) -> Self::Balance {
		0
	}

	fn reducible_balance(_asset: Self::AssetId, _who: &u64, _keep_alive: bool) -> Self::Balance {
		0
	}

	fn can_deposit(
		_asset: Self::AssetId,
		_who: &u64,
		_amount: Self::Balance,
		_mint: bool,
	) -> DepositConsequence {
		DepositConsequence::Success
	}

	fn can_withdraw(
		_asset: Self::AssetId,
		_who: &u64,
		_amount: Self::Balance,
	) -> WithdrawConsequence<Self::Balance> {
		WithdrawConsequence::Success
	}
}

impl Mutate<u64> for TestAssetProvider {
	fn mint_into(_asset: u32, _who: &u64, _amount: u128) -> DispatchResult {
		Ok(())
	}

	fn burn_from(_asset: u32, _who: &u64, _amount: u128) -> Result<u128, DispatchError> {
		Ok(0)
	}

	fn slash(_asset: u32, _who: &u64, _amount: u128) -> Result<u128, DispatchError> {
		Ok(0)
	}
}

impl MetadataInspect<u64> for TestAssetProvider {
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

impl MetadataMutate<u64> for TestAssetProvider {
	fn set(
		_asset: u32,
		_from: &u64,
		_name: Vec<u8>,
		_symbol: Vec<u8>,
		_decimals: u8,
	) -> DispatchResult {
		Ok(())
	}
}

impl Transfer<u64> for TestAssetProvider {
	fn transfer(
		asset: u32,
		source: &u64,
		dest: &u64,
		amount: u128,
		keep_alive: bool,
	) -> Result<Self::Balance, DispatchError> {
		Ok(amount)
	}
}

pub struct TestTreasuryProvider;
impl ApproveTreasuryPropose<u32, u64, H256> for TestTreasuryProvider {
	fn approve_treasury_propose(
		dao_id: u32,
		hash: H256,
		approve: bool,
	) -> Result<(), DispatchError> {
		Ok(())
	}
}

impl pallet_dao::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type PalletId = DaoPalletId;
	type Currency = pallet_balances::Pallet<Test>;
	type DaoStringLimit = ConstU32<20>;
	type DaoMetadataLimit = ConstU32<20>;
	type ExpectedBlockTime = ConstU64<3000>;
	type AssetId = u32;
	type Balance = u128;
	type CouncilProvider = TestCouncilProvider;
	type AssetProvider = TestAssetProvider;
	type DaoTokenMinBalanceLimit = ConstU128<200>;
	type DaoTokenBalanceLimit = ConstU128<20>;
	type DaoTokenVotingMinThreshold = ConstU128<20>;
	type CouncilApproveProvider = TestCouncilProvider;
	type AuthorityId = crypto::TestAuthId;
	type DaoMaxCouncilMembers = ConstU32<20>;
	type ApproveTreasuryPropose = TestTreasuryProvider;
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
			"proposal_bond": 1,
			"proposal_bond_min": 1,
			"proposal_period": 300000,
			"approve_origin": [
				1,
				2
			],
			"reject_origin": [
				1,
				2
			]
		},
		"token": {
			"token_id": TokenId::get(),
			"min_balance": TokenMinBalance::get(),
			"metadata": {
				"name": TokenName::get(),
				"symbol": TokenSymbol::get(),
				"decimals": TokenDecimals::get()
			}
		}
	})
}
