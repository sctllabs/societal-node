//! bounties pallet tests.

#![cfg(test)]

use super::*;
use crate as pallet_dao_bounties;

use frame_support::{
	assert_noop, assert_ok, parameter_types,
	traits::{AsEnsureOriginWithArg, ConstU32, ConstU64, EnsureOriginWithArg, OnInitialize},
	PalletId,
};
use frame_system::EnsureRoot;

use dao_primitives::{
	BountyPayoutDelay, BountyUpdatePeriod, DaoOrigin, DaoPolicy, DaoPolicyProportion,
	DispatchResultWithDaoOrigin, SpendDaoFunds, TreasurySpendPeriod,
};
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BadOrigin, BlakeTwo256, IdentityLookup},
	BuildStorage, Perbill, Permill,
};

use super::Event as BountiesEvent;

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
		Bounties: pallet_dao_bounties::{Pallet, Call, Storage, Event<T>},
		Bounties1: pallet_dao_bounties::<Instance1>::{Pallet, Call, Storage, Event<T>},
		Treasury: pallet_dao_treasury::{Pallet, Call, Storage, Event<T>},
		Treasury1: pallet_dao_treasury::<Instance1>::{Pallet, Call, Storage, Event<T>},
		Assets: pallet_dao_assets::{Pallet, Call, Storage, Event<T>}
	}
);

parameter_types! {
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}

type Balance = u64;

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
	type AccountId = u128; // u64 is not enough to hold bytes used to generate bounty account
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
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_balances::Config for Test {
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type Balance = Balance;
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU64<1>;
	type AccountStore = System;
	type WeightInfo = ();
}

parameter_types! {
	pub const AssetDeposit: Balance = 0;
	pub const ApprovalDeposit: Balance = 0;
	pub const StringLimit: u32 = 50;
	pub const MetadataDepositBase: Balance = 0;
	pub const MetadataDepositPerByte: Balance = 0;
	pub const MaxLocks: u32 = 10;
}
impl pallet_dao_assets::Config for Test {
	type MaxLocks = ();
	type Balance = Balance;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type AssetId = u128;
	type Currency = pallet_balances::Pallet<Test>;
	type ForceOrigin = EnsureRoot<AccountId>;
	type AssetDeposit = AssetDeposit;
	type AssetAccountDeposit = ConstU64<100>;
	type MetadataDepositBase = MetadataDepositBase;
	type MetadataDepositPerByte = MetadataDepositPerByte;
	type ApprovalDeposit = ();
	type StringLimit = StringLimit;
	type Freezer = Assets;
	type Extra = ();
	type RemoveItemsLimit = ();
	type AssetIdParameter = u128;
	type CreateOrigin = AsEnsureOriginWithArg<frame_system::EnsureSigned<u128>>;
	type CallbackHandle = ();
}

pub struct TestDaoProvider;
impl DaoProvider<H256> for TestDaoProvider {
	type Id = u32;
	type AccountId = u128;
	type AssetId = u128;
	type Policy = DaoPolicy;
	type Origin = RuntimeOrigin;
	type ApproveOrigin = AsEnsureOriginWithArg<frame_system::EnsureRoot<u128>>;

	fn exists(_id: Self::Id) -> Result<(), DispatchError> {
		Ok(())
	}

	fn dao_account_id(id: Self::Id) -> Self::AccountId {
		PalletId(*b"py/sctld").into_sub_account_truncating(id)
	}

	fn dao_token(_id: Self::Id) -> Result<DaoToken<Self::AssetId, Vec<u8>>, DispatchError> {
		Ok(DaoToken::FungibleToken(1))
	}

	fn policy(_id: Self::Id) -> Result<Self::Policy, DispatchError> {
		Ok(DaoPolicy {
			proposal_period: 3,
			approve_origin: DaoPolicyProportion::AtLeast((1, 2)),
			governance: None,
			bounty_payout_delay: BountyPayoutDelay(10),
			bounty_update_period: BountyUpdatePeriod(10),
			spend_period: TreasurySpendPeriod(100),
		})
	}

	fn count() -> u32 {
		1
	}

	fn ensure_member(_id: Self::Id, _who: &Self::AccountId) -> Result<bool, DispatchError> {
		Ok(true)
	}

	fn ensure_approved(
		origin: Self::Origin,
		dao_id: Self::Id,
	) -> DispatchResultWithDaoOrigin<Self::AccountId> {
		let dao_account_id = Self::dao_account_id(dao_id);
		let approve_origin = Self::policy(dao_id)?.approve_origin;
		let dao_origin = DaoOrigin { dao_account_id, proportion: approve_origin };

		Self::ApproveOrigin::ensure_origin(origin, &dao_origin)?;

		Ok(dao_origin)
	}
}

parameter_types! {
	pub static Burn: Permill = Permill::from_percent(50);
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
	pub const TreasuryPalletId2: PalletId = PalletId(*b"py/trsr2");
}

impl pallet_dao_treasury::Config for Test {
	type PalletId = TreasuryPalletId;
	type Currency = pallet_balances::Pallet<Test>;
	type ApproveOrigin = AsEnsureOriginWithArg<frame_system::EnsureRoot<u128>>;
	type RuntimeEvent = RuntimeEvent;
	type OnSlash = ();
	type Burn = Burn;
	type BurnDestination = (); // Just gets burned.
	type WeightInfo = ();
	type SpendFunds = Bounties;
	type MaxApprovals = ConstU32<100>;
	type AssetId = u128;
	type DaoProvider = TestDaoProvider;
	type AssetProvider = Assets;
}

impl pallet_dao_treasury::Config<Instance1> for Test {
	type PalletId = TreasuryPalletId2;
	type Currency = pallet_balances::Pallet<Test>;
	type ApproveOrigin = AsEnsureOriginWithArg<frame_system::EnsureRoot<u128>>;
	type RuntimeEvent = RuntimeEvent;
	type OnSlash = ();
	type Burn = Burn;
	type BurnDestination = (); // Just gets burned.
	type WeightInfo = ();
	type SpendFunds = Bounties1;
	type MaxApprovals = ConstU32<100>;
	type AssetId = u128;
	type DaoProvider = TestDaoProvider;
	type AssetProvider = Assets;
}

impl Config for Test {
	type Assets = Assets;
	type RuntimeEvent = RuntimeEvent;
	type MaximumReasonLength = ConstU32<16384>;
	type WeightInfo = ();
	type ChildBountyManager = ();
}

impl Config<Instance1> for Test {
	type Assets = Assets;
	type RuntimeEvent = RuntimeEvent;
	type MaximumReasonLength = ConstU32<16384>;
	type WeightInfo = ();
	type ChildBountyManager = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext: sp_io::TestExternalities = GenesisConfig {
		system: frame_system::GenesisConfig::default(),
		balances: pallet_balances::GenesisConfig { balances: vec![(0, 100), (1, 98), (2, 1)] },
	}
	.build_storage()
	.unwrap()
	.into();
	ext.execute_with(|| System::set_block_number(1));
	ext
}

fn last_event() -> BountiesEvent<Test> {
	System::events()
		.into_iter()
		.map(|r| r.event)
		.filter_map(|e| if let RuntimeEvent::Bounties(inner) = e { Some(inner) } else { None })
		.last()
		.unwrap()
}

#[test]
fn genesis_config_works() {
	new_test_ext().execute_with(|| {
		assert_eq!(Treasury::pot(0), 0);
	});
}

#[test]
fn accepted_spend_proposal_ignored_outside_spend_period() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&TestDaoProvider::dao_account_id(0), 101);

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
		Balances::make_free_balance_be(&TestDaoProvider::dao_account_id(0), 101);
		assert_eq!(Balances::total_issuance(), init_total_issuance + 101);

		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Treasury::pot(0), 100);
		assert_eq!(Balances::total_issuance(), init_total_issuance + 101);
	});
}

#[test]
fn accepted_spend_proposal_enacted_on_spend_period() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&TestDaoProvider::dao_account_id(0), 101);
		assert_eq!(Treasury::pot(0), 100);

		assert_ok!(Treasury::spend(RuntimeOrigin::root(), 0, 100, 3));

		<Treasury as OnInitialize<u64>>::on_initialize(2);
		Treasury::spend_dao_funds(0);
		assert_eq!(Balances::free_balance(3), 100);
		assert_eq!(Treasury::pot(0), 0);
	});
}

#[test]
fn pot_underflow_should_not_diminish() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&TestDaoProvider::dao_account_id(0), 101);
		assert_eq!(Treasury::pot(0), 100);

		assert_ok!(Treasury::spend(RuntimeOrigin::root(), 0, 150, 3));

		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Treasury::pot(0), 100); // Pot hasn't changed

		assert_ok!(Balances::deposit_into_existing(&TestDaoProvider::dao_account_id(0), 100));
		<Treasury as OnInitialize<u64>>::on_initialize(4);
		Treasury::spend_dao_funds(0);
		assert_eq!(Balances::free_balance(3), 150); // Fund has been spent
		assert_eq!(Treasury::pot(0), 50); // Pot has finally changed
	});
}

// Treasury account doesn't get deleted if amount approved to spend is all its free balance.
// i.e. pot should not include existential deposit needed for account survival.
#[test]
fn treasury_account_doesnt_get_deleted() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&TestDaoProvider::dao_account_id(0), 101);
		assert_eq!(Treasury::pot(0), 100);
		let treasury_balance = Balances::free_balance(&TestDaoProvider::dao_account_id(0));

		assert_ok!(Treasury::spend(RuntimeOrigin::root(), 0, treasury_balance, 3));

		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Treasury::pot(0), 100); // Pot hasn't changed

		assert_ok!(Treasury::spend(RuntimeOrigin::root(), 0, Treasury::pot(0), 3));

		<Treasury as OnInitialize<u64>>::on_initialize(4);
		Treasury::spend_dao_funds(0);
		assert_eq!(Treasury::pot(0), 0); // Pot is emptied
		assert_eq!(Balances::free_balance(TestDaoProvider::dao_account_id(0)), 1); // but the account is
		                                                                   // still there
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
		assert_eq!(Balances::free_balance(TestDaoProvider::dao_account_id(0)), 0); // Account does not exist
		assert_eq!(Treasury::pot(0), 0); // Pot is empty

		assert_ok!(Treasury::spend(RuntimeOrigin::root(), 0, 99, 3));
		assert_ok!(Treasury::spend(RuntimeOrigin::root(), 0, 1, 3));
		<Treasury as OnInitialize<u64>>::on_initialize(2);
		assert_eq!(Treasury::pot(0), 0); // Pot hasn't changed
		assert_eq!(Balances::free_balance(3), 0); // Balance of `3` hasn't changed

		Balances::make_free_balance_be(&TestDaoProvider::dao_account_id(0), 100);
		assert_eq!(Treasury::pot(0), 99); // Pot now contains funds
		assert_eq!(Balances::free_balance(TestDaoProvider::dao_account_id(0)), 100); // Account does exist

		<Treasury as OnInitialize<u64>>::on_initialize(4);
		Treasury::spend_dao_funds(0);

		assert_eq!(Treasury::pot(0), 0); // Pot has changed
		assert_eq!(Balances::free_balance(3), 99); // Balance of `3` has changed
	});
}

#[test]
fn close_bounty_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		Balances::make_free_balance_be(&TestDaoProvider::dao_account_id(0), 101);
		assert_noop!(
			Bounties::close_bounty(RuntimeOrigin::root(), 0, 0),
			Error::<Test>::InvalidIndex
		);

		assert_ok!(Bounties::create_bounty(RuntimeOrigin::root(), 0, 10, b"12345".to_vec()));

		assert_eq!(
			last_event(),
			BountiesEvent::BountyCreated {
				dao_id: 0,
				index: 0,
				status: BountyStatus::Approved,
				description: b"12345".to_vec(),
				value: 10,
				token_id: None,
			}
		);

		assert_eq!(Balances::reserved_balance(0), 0);
		assert_eq!(Balances::free_balance(0), 100);

		// TODO: re-visit this
		// assert_eq!(Bounties::bounties(0, 0), None);
		// assert!(!pallet_dao_treasury::Proposals::<Test>::contains_key(0, 0));
		//
		// assert_eq!(Bounties::bounty_descriptions(0, 0), None);
	});
}

#[test]
fn create_bounty_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		Balances::make_free_balance_be(&TestDaoProvider::dao_account_id(0), 101);

		assert_ok!(Bounties::create_bounty(RuntimeOrigin::root(), 0, 50, b"12345".to_vec()));

		let deposit: u64 = 0;

		assert_eq!(
			Bounties::bounties(0, 0).unwrap(),
			Bounty {
				token_id: None,
				fee: 0,
				value: 50,
				curator_deposit: 0,
				status: BountyStatus::Approved,
			}
		);
		assert_eq!(Bounties::bounty_approvals(0), vec![0]);

		assert_noop!(
			Bounties::close_bounty(RuntimeOrigin::root(), 0, 0),
			Error::<Test>::UnexpectedStatus
		);

		// deposit not returned yet
		assert_eq!(Balances::reserved_balance(0), 0);
		assert_eq!(Balances::free_balance(0), 100 - deposit);

		<Treasury as OnInitialize<u64>>::on_initialize(2);
		Treasury::spend_dao_funds(0);

		// return deposit
		assert_eq!(Balances::reserved_balance(0), 0);
		assert_eq!(Balances::free_balance(0), 100);

		assert_eq!(
			Bounties::bounties(0, 0).unwrap(),
			Bounty {
				token_id: None,
				fee: 0,
				curator_deposit: 0,
				value: 50,
				status: BountyStatus::Funded,
			}
		);

		assert_eq!(Treasury::pot(0), 100 - 50); // burn 25
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0, 0)), 50);
	});
}

#[test]
fn assign_curator_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		Balances::make_free_balance_be(&TestDaoProvider::dao_account_id(0), 101);

		assert_noop!(
			Bounties::propose_curator(RuntimeOrigin::root(), 0, 0, 4, 4),
			Error::<Test>::InvalidIndex
		);

		assert_ok!(Bounties::create_bounty(RuntimeOrigin::root(), 0, 50, b"12345".to_vec()));

		System::set_block_number(2);
		Treasury::spend_dao_funds(0);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_noop!(
			Bounties::propose_curator(RuntimeOrigin::root(), 0, 0, 4, 50),
			Error::<Test>::InvalidFee
		);

		let fee = 0;
		assert_ok!(Bounties::propose_curator(RuntimeOrigin::root(), 0, 0, 4, fee));

		assert_eq!(
			Bounties::bounties(0, 0).unwrap(),
			Bounty {
				token_id: None,
				fee,
				curator_deposit: 0,
				value: 50,
				status: BountyStatus::CuratorProposed { curator: 4 },
			}
		);

		assert_noop!(
			Bounties::accept_curator(RuntimeOrigin::signed(1), 0, 0),
			Error::<Test>::RequireCurator
		);

		Balances::make_free_balance_be(&4, 10);

		assert_ok!(Bounties::accept_curator(RuntimeOrigin::signed(4), 0, 0));

		assert_eq!(
			Bounties::bounties(0, 0).unwrap(),
			Bounty {
				token_id: None,
				fee,
				curator_deposit: fee,
				value: 50,
				status: BountyStatus::Active { curator: 4, update_due: 12 },
			}
		);

		assert_eq!(Balances::free_balance(&4), 10 - fee);
		assert_eq!(Balances::reserved_balance(&4), fee);
	});
}

#[test]
fn unassign_curator_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		Balances::make_free_balance_be(&TestDaoProvider::dao_account_id(0), 101);
		assert_ok!(Bounties::create_bounty(RuntimeOrigin::root(), 0, 50, b"12345".to_vec()));

		System::set_block_number(2);
		Treasury::spend_dao_funds(0);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		let fee = 0;

		assert_ok!(Bounties::propose_curator(RuntimeOrigin::root(), 0, 0, 4, fee));
		assert_noop!(Bounties::unassign_curator(RuntimeOrigin::signed(1), 0, 0), BadOrigin);
		assert_ok!(Bounties::unassign_curator(RuntimeOrigin::signed(4), 0, 0));

		assert_eq!(
			Bounties::bounties(0, 0).unwrap(),
			Bounty {
				token_id: None,
				fee,
				curator_deposit: 0,
				value: 50,
				status: BountyStatus::Funded,
			}
		);

		assert_ok!(Bounties::propose_curator(RuntimeOrigin::root(), 0, 0, 4, fee));
		Balances::make_free_balance_be(&4, 10);
		assert_ok!(Bounties::accept_curator(RuntimeOrigin::signed(4), 0, 0));
		assert_ok!(Bounties::unassign_curator(RuntimeOrigin::root(), 0, 0));

		assert_eq!(
			Bounties::bounties(0, 0).unwrap(),
			Bounty {
				token_id: None,
				fee,
				curator_deposit: 0,
				value: 50,
				status: BountyStatus::Funded,
			}
		);

		assert_eq!(Balances::free_balance(&4), 10 - fee);
		assert_eq!(Balances::reserved_balance(&4), 0); // slashed curator deposit
	});
}

#[test]
fn award_and_claim_bounty_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		Balances::make_free_balance_be(&TestDaoProvider::dao_account_id(0), 101);
		Balances::make_free_balance_be(&4, 10);
		assert_ok!(Bounties::create_bounty(RuntimeOrigin::root(), 0, 50, b"12345".to_vec()));

		System::set_block_number(2);
		Treasury::spend_dao_funds(0);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		let fee = 0;
		assert_ok!(Bounties::propose_curator(RuntimeOrigin::root(), 0, 0, 4, fee));
		assert_ok!(Bounties::accept_curator(RuntimeOrigin::signed(4), 0, 0));

		assert_eq!(Balances::free_balance(4), 10 - fee);

		assert_noop!(
			Bounties::award_bounty(RuntimeOrigin::signed(1), 0, 0, 3),
			Error::<Test>::RequireCurator
		);

		assert_ok!(Bounties::award_bounty(RuntimeOrigin::signed(4), 0, 0, 3));

		assert_eq!(
			Bounties::bounties(0, 0).unwrap(),
			Bounty {
				token_id: None,
				fee,
				curator_deposit: fee,
				value: 50,
				status: BountyStatus::PendingPayout { curator: 4, beneficiary: 3, unlock_at: 12 },
			}
		);

		assert_noop!(
			Bounties::claim_bounty(RuntimeOrigin::signed(1), 0, 0),
			Error::<Test>::Premature
		);

		System::set_block_number(12);
		Treasury::spend_dao_funds(0);
		<Treasury as OnInitialize<u64>>::on_initialize(5);

		assert_ok!(Balances::transfer(
			RuntimeOrigin::signed(0),
			Bounties::bounty_account_id(0, 0),
			10
		));

		assert_ok!(Bounties::claim_bounty(RuntimeOrigin::signed(1), 0, 0));

		assert_eq!(
			last_event(),
			BountiesEvent::BountyClaimed { dao_id: 0, index: 0, payout: 60, beneficiary: 3 }
		);

		assert_eq!(Balances::free_balance(4), 10); // initial 10 + fee 4

		assert_eq!(Balances::free_balance(3), 60);
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0, 0)), 0);

		assert_eq!(Bounties::bounties(0, 0), None);
		assert_eq!(Bounties::bounty_descriptions(0, 0), None);
	});
}

#[test]
fn claim_handles_high_fee() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		Balances::make_free_balance_be(&TestDaoProvider::dao_account_id(0), 101);
		Balances::make_free_balance_be(&4, 30);
		assert_ok!(Bounties::create_bounty(RuntimeOrigin::root(), 0, 50, b"12345".to_vec()));

		System::set_block_number(2);
		Treasury::spend_dao_funds(0);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_ok!(Bounties::propose_curator(RuntimeOrigin::root(), 0, 0, 4, 0));
		assert_ok!(Bounties::accept_curator(RuntimeOrigin::signed(4), 0, 0));

		assert_ok!(Bounties::award_bounty(RuntimeOrigin::signed(4), 0, 0, 3));

		System::set_block_number(15);
		Treasury::spend_dao_funds(0);
		<Treasury as OnInitialize<u64>>::on_initialize(5);

		// make fee > balance
		let res = Balances::slash(&Bounties::bounty_account_id(0, 0), 10);
		assert_eq!(res.0.peek(), 10);

		assert_ok!(Bounties::claim_bounty(RuntimeOrigin::signed(1), 0, 0));

		assert_eq!(
			last_event(),
			BountiesEvent::BountyClaimed { dao_id: 0, index: 0, payout: 40, beneficiary: 3 }
		);

		assert_eq!(Balances::free_balance(4), 30); // 30 + 50 - 10
		assert_eq!(Balances::free_balance(3), 40);
		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0, 0)), 0);

		assert_eq!(Bounties::bounties(0, 0), None);
		assert_eq!(Bounties::bounty_descriptions(0, 0), None);
	});
}

#[test]
fn cancel_and_refund() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);

		Balances::make_free_balance_be(&TestDaoProvider::dao_account_id(0), 101);

		assert_ok!(Bounties::create_bounty(RuntimeOrigin::root(), 0, 50, b"12345".to_vec()));

		System::set_block_number(2);
		Treasury::spend_dao_funds(0);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_ok!(Balances::transfer(
			RuntimeOrigin::signed(0),
			Bounties::bounty_account_id(0, 0),
			10
		));

		assert_eq!(
			Bounties::bounties(0, 0).unwrap(),
			Bounty {
				token_id: None,
				fee: 0,
				curator_deposit: 0,
				value: 50,
				status: BountyStatus::Funded,
			}
		);

		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0, 0)), 60);

		assert_noop!(Bounties::close_bounty(RuntimeOrigin::signed(0), 0, 0), BadOrigin);

		assert_ok!(Bounties::close_bounty(RuntimeOrigin::root(), 0, 0));

		// `- 25 + 10`
		assert_eq!(Treasury::pot(0), 110);
	});
}

#[test]
fn award_and_cancel() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		Balances::make_free_balance_be(&TestDaoProvider::dao_account_id(0), 101);
		assert_ok!(Bounties::create_bounty(RuntimeOrigin::root(), 0, 50, b"12345".to_vec()));

		System::set_block_number(2);
		Treasury::spend_dao_funds(0);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_ok!(Bounties::propose_curator(RuntimeOrigin::root(), 0, 0, 0, 0));
		assert_ok!(Bounties::accept_curator(RuntimeOrigin::signed(0), 0, 0));

		assert_eq!(Balances::free_balance(0), 100);
		assert_eq!(Balances::reserved_balance(0), 0);

		assert_ok!(Bounties::award_bounty(RuntimeOrigin::signed(0), 0, 0, 3));

		// Cannot close bounty directly when payout is happening...
		assert_noop!(
			Bounties::close_bounty(RuntimeOrigin::root(), 0, 0),
			Error::<Test>::PendingPayout
		);

		// Instead unassign the curator to slash them and then close.
		assert_ok!(Bounties::unassign_curator(RuntimeOrigin::root(), 0, 0));
		assert_ok!(Bounties::close_bounty(RuntimeOrigin::root(), 0, 0));

		assert_eq!(last_event(), BountiesEvent::BountyCanceled { dao_id: 0, index: 0 });

		assert_eq!(Balances::free_balance(Bounties::bounty_account_id(0, 0)), 0);

		// Slashed.
		assert_eq!(Balances::free_balance(0), 100);
		assert_eq!(Balances::reserved_balance(0), 0);

		assert_eq!(Bounties::bounties(0, 0), None);
		assert_eq!(Bounties::bounty_descriptions(0, 0), None);
	});
}

#[test]
fn expire_and_unassign() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		Balances::make_free_balance_be(&TestDaoProvider::dao_account_id(0), 101);
		assert_ok!(Bounties::create_bounty(RuntimeOrigin::root(), 0, 50, b"12345".to_vec()));

		System::set_block_number(2);
		Treasury::spend_dao_funds(0);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_ok!(Bounties::propose_curator(RuntimeOrigin::root(), 0, 0, 1, 0));
		assert_ok!(Bounties::accept_curator(RuntimeOrigin::signed(1), 0, 0));

		assert_eq!(Balances::free_balance(1), 98);
		assert_eq!(Balances::reserved_balance(1), 0);

		System::set_block_number(22);
		Treasury::spend_dao_funds(0);
		<Treasury as OnInitialize<u64>>::on_initialize(22);

		System::set_block_number(23);
		<Treasury as OnInitialize<u64>>::on_initialize(23);

		assert_ok!(Bounties::unassign_curator(RuntimeOrigin::signed(0), 0, 0));

		assert_eq!(
			Bounties::bounties(0, 0).unwrap(),
			Bounty {
				token_id: None,
				fee: 0,
				curator_deposit: 0,
				value: 50,
				status: BountyStatus::Funded,
			}
		);

		assert_eq!(Balances::free_balance(1), 98);
		assert_eq!(Balances::reserved_balance(1), 0); // slashed
	});
}

#[test]
fn extend_expiry() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		Balances::make_free_balance_be(&TestDaoProvider::dao_account_id(0), 101);
		Balances::make_free_balance_be(&4, 10);
		assert_ok!(Bounties::create_bounty(RuntimeOrigin::root(), 0, 50, b"12345".to_vec()));

		assert_noop!(
			Bounties::extend_bounty_expiry(RuntimeOrigin::signed(1), 0, 0, Vec::new()),
			Error::<Test>::UnexpectedStatus
		);

		System::set_block_number(2);
		Treasury::spend_dao_funds(0);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_ok!(Bounties::propose_curator(RuntimeOrigin::root(), 0, 0, 4, 0));
		assert_ok!(Bounties::accept_curator(RuntimeOrigin::signed(4), 0, 0));

		assert_eq!(Balances::free_balance(4), 10);
		assert_eq!(Balances::reserved_balance(4), 0);

		System::set_block_number(10);
		Treasury::spend_dao_funds(0);
		<Treasury as OnInitialize<u64>>::on_initialize(10);

		assert_noop!(
			Bounties::extend_bounty_expiry(RuntimeOrigin::signed(0), 0, 0, Vec::new()),
			Error::<Test>::RequireCurator
		);
		assert_ok!(Bounties::extend_bounty_expiry(RuntimeOrigin::signed(4), 0, 0, Vec::new()));

		assert_eq!(
			Bounties::bounties(0, 0).unwrap(),
			Bounty {
				token_id: None,
				fee: 0,
				curator_deposit: 0,
				value: 50,
				status: BountyStatus::Active { curator: 4, update_due: 20 },
			}
		);

		assert_ok!(Bounties::extend_bounty_expiry(RuntimeOrigin::signed(4), 0, 0, Vec::new()));

		assert_eq!(
			Bounties::bounties(0, 0).unwrap(),
			Bounty {
				token_id: None,
				fee: 0,
				curator_deposit: 0,
				value: 50,
				status: BountyStatus::Active { curator: 4, update_due: 20 }, // still the same
			}
		);

		System::set_block_number(25);
		Treasury::spend_dao_funds(0);
		<Treasury as OnInitialize<u64>>::on_initialize(25);

		assert_ok!(Bounties::unassign_curator(RuntimeOrigin::signed(4), 0, 0));

		assert_eq!(Balances::free_balance(4), 10); // not slashed
		assert_eq!(Balances::reserved_balance(4), 0);
	});
}

#[test]
fn genesis_funding_works() {
	let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
	let initial_funding = 100;
	pallet_balances::GenesisConfig::<Test> {
		// Total issuance will be 200 with treasury account initialized with 100.
		balances: vec![(0, 100), (TestDaoProvider::dao_account_id(0), initial_funding)],
	}
	.assimilate_storage(&mut t)
	.unwrap();
	let mut t: sp_io::TestExternalities = t.into();

	t.execute_with(|| {
		assert_eq!(Balances::free_balance(TestDaoProvider::dao_account_id(0)), initial_funding);
		assert_eq!(Treasury::pot(0), initial_funding - Balances::minimum_balance());
	});
}

#[test]
fn unassign_curator_self() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		Balances::make_free_balance_be(&TestDaoProvider::dao_account_id(0), 101);
		assert_ok!(Bounties::create_bounty(RuntimeOrigin::root(), 0, 50, b"12345".to_vec()));

		System::set_block_number(2);
		Treasury::spend_dao_funds(0);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_ok!(Bounties::propose_curator(RuntimeOrigin::root(), 0, 0, 1, 0));
		assert_ok!(Bounties::accept_curator(RuntimeOrigin::signed(1), 0, 0));

		assert_eq!(Balances::free_balance(1), 98);
		assert_eq!(Balances::reserved_balance(1), 0);

		System::set_block_number(8);
		Treasury::spend_dao_funds(0);
		<Treasury as OnInitialize<u64>>::on_initialize(8);

		assert_ok!(Bounties::unassign_curator(RuntimeOrigin::signed(1), 0, 0));

		assert_eq!(
			Bounties::bounties(0, 0).unwrap(),
			Bounty {
				token_id: None,
				fee: 0,
				curator_deposit: 0,
				value: 50,
				status: BountyStatus::Funded,
			}
		);

		assert_eq!(Balances::free_balance(1), 98);
		assert_eq!(Balances::reserved_balance(1), 0); // not slashed
	});
}

#[test]
fn accept_curator_handles_different_deposit_calculations() {
	// This test will verify that a bounty with and without a fee results
	// in a different curator deposit: one using the value, and one using the fee.
	new_test_ext().execute_with(|| {
		// Case 1: With a fee
		let user = 1;
		let bounty_index = 0;
		let value = 88;
		let fee = 0;

		System::set_block_number(1);
		Balances::make_free_balance_be(&TestDaoProvider::dao_account_id(0), 101);
		Balances::make_free_balance_be(&user, 100);
		assert_ok!(Bounties::create_bounty(RuntimeOrigin::root(), 0, value, b"12345".to_vec()));

		System::set_block_number(2);
		Treasury::spend_dao_funds(0);
		<Treasury as OnInitialize<u64>>::on_initialize(2);

		assert_ok!(Bounties::propose_curator(RuntimeOrigin::root(), 0, bounty_index, user, fee));
		assert_ok!(Bounties::accept_curator(RuntimeOrigin::signed(user), 0, bounty_index));

		let expected_deposit = fee;
		assert_eq!(Balances::free_balance(&user), 100 - expected_deposit);
		assert_eq!(Balances::reserved_balance(&user), expected_deposit);

		// Case 2: Lower bound
		let user = 2;
		let bounty_index = 1;
		let value = 35;
		let fee = 0;

		Balances::make_free_balance_be(&TestDaoProvider::dao_account_id(0), 101);
		Balances::make_free_balance_be(&user, 100);

		assert_ok!(Bounties::create_bounty(RuntimeOrigin::root(), 0, value, b"12345".to_vec()));

		System::set_block_number(4);
		Treasury::spend_dao_funds(0);
		<Treasury as OnInitialize<u64>>::on_initialize(4);

		assert_ok!(Bounties::propose_curator(RuntimeOrigin::root(), 0, bounty_index, user, fee));
		assert_ok!(Bounties::accept_curator(RuntimeOrigin::signed(user), 0, bounty_index));

		let expected_deposit = 0;
		assert_eq!(Balances::free_balance(&user), 100 - expected_deposit);
		assert_eq!(Balances::reserved_balance(&user), expected_deposit);

		// Case 3: Upper bound
		let user = 3;
		let bounty_index = 2;
		let value = 1_000_000;
		let fee = 0;
		let starting_balance = fee * 2;

		Balances::make_free_balance_be(&TestDaoProvider::dao_account_id(0), value * 2);
		Balances::make_free_balance_be(&user, starting_balance);
		Balances::make_free_balance_be(&0, starting_balance);

		assert_ok!(Bounties::create_bounty(RuntimeOrigin::root(), 0, value, b"12345".to_vec()));

		System::set_block_number(6);
		Treasury::spend_dao_funds(0);
		<Treasury as OnInitialize<u64>>::on_initialize(6);

		assert_ok!(Bounties::propose_curator(RuntimeOrigin::root(), 0, bounty_index, user, fee));
		assert_ok!(Bounties::accept_curator(RuntimeOrigin::signed(user), 0, bounty_index));

		println!("balance: {:?}", starting_balance);
		println!("deposit: {:?}", expected_deposit);
		println!("reserved: {:?}", starting_balance.checked_sub(expected_deposit).unwrap_or(0));
		let expected_deposit = 0;
		assert_eq!(
			Balances::free_balance(&user),
			starting_balance.checked_sub(expected_deposit).unwrap_or(0)
		);
		assert_eq!(Balances::reserved_balance(&user), expected_deposit);
	});
}
