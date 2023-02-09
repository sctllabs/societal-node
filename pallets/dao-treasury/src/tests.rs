// Changes made comparing to the original tests of the FRAME pallet-treasure:
// - using DAO as a parameter for pallet functions

//! DAO Treasury pallet tests.

#![cfg(test)]

use std::cell::RefCell;

use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BadOrigin, BlakeTwo256, IdentityLookup},
};

use dao_primitives::{AccountTokenBalance, DaoPolicyProportion};
use frame_support::{
	assert_noop, assert_ok,
	dispatch::DispatchError,
	parameter_types,
	traits::{
		tokens::{
			fungibles::{
				metadata::{Inspect as MetadataInspect, Mutate as MetadataMutate},
				Create, Inspect, Mutate,
			},
			DepositConsequence, WithdrawConsequence,
		},
		AsEnsureOriginWithArg, ConstU128, ConstU32, ConstU64, OnInitialize,
	},
	PalletId,
};

use super::*;
use crate as treasury;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
type AccountId = u128;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Treasury: treasury::{Pallet, Call, Storage, Event<T>},
	}
);

parameter_types! {
	pub BlockWeights: frame_system::limits::BlockWeights =
		frame_system::limits::BlockWeights::simple_max(Weight::from_ref_time(1024));
}
impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type RuntimeOrigin = RuntimeOrigin;
	type Index = u64;
	type BlockNumber = u64;
	type RuntimeCall = RuntimeCall;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId; // u64 is not enough to hold bytes used to generate bounty account
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
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
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
thread_local! {
	static TEN_TO_FOURTEEN: RefCell<Vec<u128>> = RefCell::new(vec![10,11,12,13,14]);
}
parameter_types! {
	pub const Burn: Permill = Permill::from_percent(50);
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
	pub const DaoPalletId: PalletId = PalletId(*b"py/sctld");

	pub TokenId: u32 = 0;
	pub TokenName: String = "dao_token".into();
	pub TokenSymbol: String = "sctl".into();
	pub TokenDecimals: u8 = 3;
	pub TokenMinBalance: String = "1000000000".into();
}
pub struct TestSpendOrigin;
impl frame_support::traits::EnsureOrigin<RuntimeOrigin> for TestSpendOrigin {
	type Success = u64;
	fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
		Result::<frame_system::RawOrigin<_>, RuntimeOrigin>::from(o).and_then(|o| match o {
			frame_system::RawOrigin::Root => Ok(u64::max_value()),
			frame_system::RawOrigin::Signed(10) => Ok(5),
			frame_system::RawOrigin::Signed(11) => Ok(10),
			frame_system::RawOrigin::Signed(12) => Ok(20),
			frame_system::RawOrigin::Signed(13) => Ok(50),
			r => Err(RuntimeOrigin::from(r)),
		})
	}
	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
		Ok(RuntimeOrigin::root())
	}
}

pub struct TestDaoProvider;
impl DaoProvider<H256> for TestDaoProvider {
	type Id = u32;
	type AccountId = u128;
	type AssetId = u32;
	type Policy = DaoPolicy;

	fn exists(_id: Self::Id) -> Result<(), DispatchError> {
		Ok(())
	}

	fn count() -> u32 {
		1
	}

	fn policy(_id: Self::Id) -> Result<Self::Policy, DispatchError> {
		Ok(DaoPolicy {
			proposal_period: 100,
			approve_origin: DaoPolicyProportion::AtLeast((3, 5)),
			governance: None,
		})
	}

	fn dao_account_id(id: Self::Id) -> Self::AccountId {
		PalletId(*b"py/sctld").into_sub_account_truncating(id)
	}

	fn ensure_member(id: Self::Id, who: &Self::AccountId) -> Result<bool, DispatchError> {
		Ok(true)
	}

	fn dao_token(id: Self::Id) -> Result<DaoToken<Self::AssetId, Vec<u8>>, DispatchError> {
		todo!()
	}

	fn ensure_eth_proposal_allowed(
		id: Self::Id,
		account_id: Vec<u8>,
		hash: H256,
		length_bound: u32,
	) -> Result<AccountTokenBalance, DispatchError> {
		Ok(AccountTokenBalance::Sufficient)
	}

	fn ensure_eth_voting_allowed(
		id: Self::Id,
		account_id: Vec<u8>,
		hash: H256,
		block_number: u32,
	) -> Result<AccountTokenBalance, DispatchError> {
		Ok(AccountTokenBalance::Sufficient)
	}

	fn ensure_eth_token_balance(id: Self::Id) -> Result<AccountTokenBalance, DispatchError> {
		Ok(AccountTokenBalance::Sufficient)
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

impl Config for Test {
	type PalletId = TreasuryPalletId;
	type Currency = pallet_balances::Pallet<Test>;
	type AssetId = u32;
	type ApproveOrigin = AsEnsureOriginWithArg<frame_system::EnsureRoot<u128>>;
	type RuntimeEvent = RuntimeEvent;
	type OnSlash = ();
	type SpendPeriod = ConstU64<2>;
	type Burn = Burn;
	type BurnDestination = (); // Just gets burned.
	type WeightInfo = ();
	type SpendFunds = ();
	type MaxApprovals = ConstU32<100>;
	type DaoProvider = TestDaoProvider;
	type AssetProvider = TestAssetProvider;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
	pallet_balances::GenesisConfig::<Test> {
		// Total issuance will be 200 with treasury account initialized at ED.
		balances: vec![(0, 100), (1, 98), (2, 1)],
	}
	.assimilate_storage(&mut t)
	.unwrap();
	t.into()
}

#[test]
fn genesis_config_works() {
	new_test_ext().execute_with(|| {
		assert_eq!(Treasury::pot(0), 0);
		assert_eq!(Treasury::proposal_count(0), 0);
	});
}

#[test]
fn spend_origin_permissioning_works() {
	new_test_ext().execute_with(|| {
		assert_noop!(Treasury::spend(RuntimeOrigin::signed(1), 0, 1, 1), BadOrigin);
		assert_noop!(Treasury::spend(RuntimeOrigin::signed(10), 0, 6, 1), BadOrigin);
		assert_noop!(Treasury::spend(RuntimeOrigin::signed(11), 0, 11, 1), BadOrigin);
		assert_noop!(Treasury::spend(RuntimeOrigin::signed(12), 0, 21, 1), BadOrigin);
		assert_noop!(Treasury::spend(RuntimeOrigin::signed(13), 0, 51, 1), BadOrigin);
	});
}

#[test]
fn spend_origin_works() {
	new_test_ext().execute_with(|| {
		// Check that accumulate works when we have Some value in Dummy already.
		Balances::make_free_balance_be(&<Test as Config>::DaoProvider::dao_account_id(0), 101);
		assert_ok!(Treasury::spend(RuntimeOrigin::root(), 0, 5, 6));
		assert_ok!(Treasury::spend(RuntimeOrigin::root(), 0, 5, 6));
		assert_ok!(Treasury::spend(RuntimeOrigin::root(), 0, 5, 6));
		assert_ok!(Treasury::spend(RuntimeOrigin::root(), 0, 5, 6));
		assert_ok!(Treasury::spend(RuntimeOrigin::root(), 0, 10, 6));
		assert_ok!(Treasury::spend(RuntimeOrigin::root(), 0, 20, 6));
		assert_ok!(Treasury::spend(RuntimeOrigin::root(), 0, 50, 6));

		<Treasury as OnInitialize<u64>>::on_initialize(1);
		assert_eq!(Balances::free_balance(6), 0);

		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Balances::free_balance(6), 100);
		assert_eq!(Treasury::pot(0), 0);
	});
}

#[test]
fn minting_works() {
	new_test_ext().execute_with(|| {
		// Check that accumulate works when we have Some value in Dummy already.
		Balances::make_free_balance_be(&<Test as Config>::DaoProvider::dao_account_id(0), 101);
		assert_eq!(Treasury::pot(0), 100);
	});
}

#[test]
fn accepted_spend_proposal_ignored_outside_spend_period() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&<Test as Config>::DaoProvider::dao_account_id(0), 101);

		assert_ok!(Treasury::spend(RuntimeOrigin::root(), 0, 100, 3));

		<Treasury as OnInitialize<u64>>::on_initialize(1);
		assert_eq!(Balances::free_balance(3), 0);
		assert_eq!(Treasury::pot(0), 100);
	});
}

#[test]
fn unused_pot_should_diminish() {
	new_test_ext().execute_with(|| {
		let init_total_issuance = Balances::total_issuance();
		Balances::make_free_balance_be(&<Test as Config>::DaoProvider::dao_account_id(0), 101);
		assert_eq!(Balances::total_issuance(), init_total_issuance + 101);

		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Treasury::pot(0), 100);
		assert_eq!(Balances::total_issuance(), init_total_issuance + 101);
	});
}

#[test]
fn accepted_spend_proposal_enacted_on_spend_period() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&<Test as Config>::DaoProvider::dao_account_id(0), 101);
		assert_eq!(Treasury::pot(0), 100);

		assert_ok!(Treasury::spend(RuntimeOrigin::root(), 0, 100, 3));

		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Balances::free_balance(3), 100);
		assert_eq!(Treasury::pot(0), 0);
	});
}

#[test]
fn pot_underflow_should_not_diminish() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&<Test as Config>::DaoProvider::dao_account_id(0), 101);
		assert_eq!(Treasury::pot(0), 100);

		assert_ok!(Treasury::spend(RuntimeOrigin::root(), 0, 150, 3));

		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Treasury::pot(0), 100); // Pot hasn't changed

		let _ =
			Balances::deposit_into_existing(&<Test as Config>::DaoProvider::dao_account_id(0), 100)
				.unwrap();
		<Treasury as OnInitialize<u64>>::on_initialize(4);
		assert_eq!(Balances::free_balance(3), 150); // Fund has been spent
		assert_eq!(Treasury::pot(0), 50); // Pot has finally changed
	});
}

// Treasury account doesn't get deleted if amount approved to spend is all its free balance.
// i.e. pot should not include existential deposit needed for account survival.
#[test]
fn treasury_account_doesnt_get_deleted() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&<Test as Config>::DaoProvider::dao_account_id(0), 101);
		assert_eq!(Treasury::pot(0), 100);
		let treasury_balance =
			Balances::free_balance(&<Test as Config>::DaoProvider::dao_account_id(0));

		assert_ok!(Treasury::spend(RuntimeOrigin::root(), 0, treasury_balance, 3));

		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Treasury::pot(0), 100); // Pot hasn't changed

		assert_ok!(Treasury::spend(RuntimeOrigin::root(), 0, Treasury::pot(0), 3));

		<Treasury as OnInitialize<u64>>::on_initialize(4);
		assert_eq!(Treasury::pot(0), 0); // Pot is emptied
		assert_eq!(Balances::free_balance(<Test as Config>::DaoProvider::dao_account_id(0)), 1); // but the account is still there
	});
}

// In case treasury account is not existing then it works fine.
// This is useful for chain that will just update runtime.
#[test]
fn inexistent_account_works() {
	let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
	pallet_balances::GenesisConfig::<Test> { balances: vec![(0, 100), (1, 99), (2, 1)] }
		.assimilate_storage(&mut t)
		.unwrap();
	// Treasury genesis config is not build thus treasury account does not exist
	let mut t: sp_io::TestExternalities = t.into();

	t.execute_with(|| {
		assert_eq!(Balances::free_balance(<Test as Config>::DaoProvider::dao_account_id(0)), 0); // Account does not exist
		assert_eq!(Treasury::pot(0), 0); // Pot is empty

		assert_ok!(Treasury::spend(RuntimeOrigin::root(), 0, 99, 3));
		assert_ok!(Treasury::spend(RuntimeOrigin::root(), 0, 1, 3));
		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Treasury::pot(0), 0); // Pot hasn't changed
		assert_eq!(Balances::free_balance(3), 0); // Balance of `3` hasn't changed

		Balances::make_free_balance_be(&<Test as Config>::DaoProvider::dao_account_id(0), 100);
		assert_eq!(Treasury::pot(0), 99); // Pot now contains funds
		assert_eq!(Balances::free_balance(<Test as Config>::DaoProvider::dao_account_id(0)), 100); // Account does exist

		<Treasury as OnInitialize<u64>>::on_initialize(4);

		assert_eq!(Treasury::pot(0), 0); // Pot has changed
		assert_eq!(Balances::free_balance(3), 99); // Balance of `3` has changed
	});
}

#[test]
fn genesis_funding_works() {
	let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
	let initial_funding = 100;
	pallet_balances::GenesisConfig::<Test> {
		// Total issuance will be 200 with treasury account initialized with 100.
		balances: vec![
			(0, 100),
			(<Test as Config>::DaoProvider::dao_account_id(0), initial_funding),
		],
	}
	.assimilate_storage(&mut t)
	.unwrap();
	let mut t: sp_io::TestExternalities = t.into();

	t.execute_with(|| {
		assert_eq!(
			Balances::free_balance(<Test as Config>::DaoProvider::dao_account_id(0)),
			initial_funding
		);
		assert_eq!(Treasury::pot(0), initial_funding - Balances::minimum_balance());
	});
}

#[test]
fn max_approvals_limited() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(
			&<Test as Config>::DaoProvider::dao_account_id(0),
			u128::MAX,
		);
		Balances::make_free_balance_be(&0, u128::MAX);

		for _ in 0..<<Test as Config>::MaxApprovals as sp_core::TypedGet>::get() {
			assert_ok!(Treasury::spend(RuntimeOrigin::root(), 0, 100, 3));
		}

		// One too many will fail
		assert_noop!(
			Treasury::spend(RuntimeOrigin::root(), 0, 100, 3),
			Error::<Test, _>::TooManyApprovals
		);
	});
}
