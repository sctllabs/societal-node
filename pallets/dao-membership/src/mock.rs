use super::*;
use crate as pallet_dao_membership;

use std::collections::HashMap;

use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{AccountIdConversion, BlakeTwo256, IdentityLookup},
};

use dao_primitives::{AccountTokenBalance, DaoPolicyProportion, DaoToken};
use frame_support::{
	ord_parameter_types, parameter_types,
	traits::{AsEnsureOriginWithArg, ConstU32, ConstU64},
	weights::Weight,
	PalletId,
};
use frame_system::EnsureSignedBy;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Membership: pallet_dao_membership::{Pallet, Call, Storage, Event<T>},
	}
);

parameter_types! {
	pub BlockWeights: frame_system::limits::BlockWeights =
		frame_system::limits::BlockWeights::simple_max(Weight::from_ref_time(1024));
	pub static Members: HashMap<u32, Vec<u64>> = HashMap::new();
}

impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type RuntimeOrigin = RuntimeOrigin;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type RuntimeCall = RuntimeCall;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}
ord_parameter_types! {
	pub const One: u64 = 1;
	pub const Two: u64 = 2;
	pub const Three: u64 = 3;
	pub const Four: u64 = 4;
	pub const Five: u64 = 5;
}

pub struct TestChangeMembers;
impl ChangeDaoMembers<u32, u64> for TestChangeMembers {
	fn change_members_sorted(dao_id: DaoId, incoming: &[u64], outgoing: &[u64], new: &[u64]) {
		let members_map = Members::get();

		let mut old_plus_incoming = members_map.get(&dao_id).unwrap().clone();
		old_plus_incoming.extend_from_slice(incoming);
		old_plus_incoming.sort();
		let mut new_plus_outgoing = new.to_vec();
		new_plus_outgoing.extend_from_slice(outgoing);
		new_plus_outgoing.sort();
		assert_eq!(*old_plus_incoming, new_plus_outgoing);

		let mut map = Members::get().clone();
		map.insert(dao_id, new.to_vec());
		Members::set(map);
	}
}

impl InitializeDaoMembers<u32, u64> for TestChangeMembers {
	fn initialize_members(dao_id: DaoId, members: Vec<u64>) -> Result<(), DispatchError> {
		let mut map = HashMap::new();
		map.insert(dao_id, members.clone());
		MEMBERS.with(|m| *m.borrow_mut() = map);

		Ok(())
	}
}

pub struct TestDaoProvider;
impl DaoProvider<H256> for TestDaoProvider {
	type Id = u32;
	type AccountId = u64;
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

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type ApproveOrigin = AsEnsureOriginWithArg<EnsureSignedBy<One, u64>>;
	type MembershipInitialized = TestChangeMembers;
	type MembershipChanged = TestChangeMembers;
	type MaxMembers = ConstU32<10>;
	type WeightInfo = ();
	type DaoProvider = TestDaoProvider;
}

pub(crate) fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext: sp_io::TestExternalities =
		frame_system::GenesisConfig::default().build_storage::<Test>().unwrap().into();
	ext.execute_with(|| {
		init_members();
	});
	ext
}

pub(crate) fn init_members() {
	let mut members: HashMap<u32, Vec<u64>> = HashMap::new();
	members.insert(0, vec![10, 20, 30]);

	Members::set(members);

	Membership::initialize_members(0, vec![10, 20, 30]).ok();
}

pub(crate) fn clean() {
	let mut members: HashMap<u32, Vec<u64>> = HashMap::new();
	members.insert(0, vec![]);

	Members::set(members);
}
