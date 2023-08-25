use crate as pallet_dao_subscription;
use dao_primitives::{DaoOrigin, DaoPolicy, DaoProvider, DaoToken, DispatchResultWithDaoOrigin};
use frame_support::{
	dispatch::DispatchError,
	parameter_types,
	traits::{
		fungibles::{
			metadata::{Inspect as MetadataInspect, Mutate as MetadataMutate},
			Create, Inspect, Mutate, Transfer,
		},
		tokens::{DepositConsequence, WithdrawConsequence},
		AsEnsureOriginWithArg, ConstU16, ConstU64, EnsureOriginWithArg,
	},
	PalletId,
};
use frame_system as system;
use sp_core::{ConstU128, ConstU32, H256};
use sp_runtime::{
	testing::Header,
	traits::{AccountIdConversion, BlakeTwo256, IdentityLookup},
	DispatchResult,
};

use crate::Error;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
type AccountId = u128;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
		Subscription: pallet_dao_subscription::{Pallet, Call, Storage, Event<T>}
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
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");

	pub TokenId: u128 = 0;
	pub TokenName: String = "dao_token".into();
	pub TokenSymbol: String = "sctl".into();
	pub TokenDecimals: u8 = 3;
	pub TokenMinBalance: String = "1000000000".into();
}

impl pallet_dao_subscription::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = pallet_balances::Pallet<Test>;
	type AssetId = u128;
	type TreasuryPalletId = TreasuryPalletId;
	type TokenBalancesLimit = ConstU32<10>;
	type AssetProvider = TestAssetProvider;
	type WeightInfo = ();
}

pub struct TestDaoProvider;
impl DaoProvider<u128, H256> for TestDaoProvider {
	type Id = u32;
	type AssetId = u128;
	type Policy = DaoPolicy;
	type Origin = RuntimeOrigin;
	type ApproveOrigin = AsEnsureOriginWithArg<frame_system::EnsureRoot<u128>>;
	type NFTCollectionId = u32;

	fn dao_account_id(id: Self::Id) -> u128 {
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
		Err(Error::<Test>::NotSupported.into())
	}

	fn ensure_approved(
		origin: Self::Origin,
		dao_id: Self::Id,
	) -> DispatchResultWithDaoOrigin<u128> {
		let dao_account_id = Self::dao_account_id(dao_id);
		let approve_origin = Self::policy(dao_id)?.approve_origin;
		let dao_origin = DaoOrigin { dao_account_id, proportion: approve_origin };

		Self::ApproveOrigin::ensure_origin(origin, &dao_origin)?;

		Ok(dao_origin)
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn create_dao(
		_founder: u128,
		_council: Vec<u128>,
		_technical_committee: Vec<u128>,
		_data: Vec<u8>,
	) -> Result<(), DispatchError> {
		Ok(())
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn approve_dao(_dao_hash: H256, _approve: bool) -> Result<(), DispatchError> {
		Ok(())
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin(dao_origin: &DaoOrigin<u128>) -> Result<Self::Origin, ()> {
		Self::ApproveOrigin::try_successful_origin(dao_origin)
	}
}

pub struct TestAssetProvider;
impl Create<AccountId> for TestAssetProvider {
	fn create(
		_id: u128,
		_admin: AccountId,
		_is_sufficient: bool,
		_min_balance: u128,
	) -> DispatchResult {
		Ok(())
	}
}

impl Inspect<AccountId> for TestAssetProvider {
	type AssetId = u128;
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

	fn asset_exists(_asset: Self::AssetId) -> bool {
		true
	}
}

impl Mutate<AccountId> for TestAssetProvider {
	fn mint_into(_asset: u128, _who: &AccountId, _amount: u128) -> DispatchResult {
		Ok(())
	}

	fn burn_from(_asset: u128, _who: &AccountId, _amount: u128) -> Result<u128, DispatchError> {
		Ok(0)
	}

	fn slash(_asset: u128, _who: &AccountId, _amount: u128) -> Result<u128, DispatchError> {
		Ok(0)
	}
}

impl MetadataInspect<AccountId> for TestAssetProvider {
	fn name(asset: u128) -> Vec<u8> {
		if asset == TokenId::get() {
			return TokenName::get().as_bytes().to_vec()
		}

		vec![]
	}

	fn symbol(asset: u128) -> Vec<u8> {
		if asset == TokenId::get() {
			return TokenSymbol::get().as_bytes().to_vec()
		}

		vec![]
	}

	fn decimals(asset: u128) -> u8 {
		if asset == TokenId::get() {
			return TokenDecimals::get()
		}

		0
	}
}

impl MetadataMutate<AccountId> for TestAssetProvider {
	fn set(
		_asset: u128,
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
		_asset: u128,
		_source: &AccountId,
		_dest: &AccountId,
		amount: u128,
		_keep_alive: bool,
	) -> Result<Self::Balance, DispatchError> {
		Ok(amount)
	}
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	system::GenesisConfig::default().build_storage::<Test>().unwrap().into()
}
