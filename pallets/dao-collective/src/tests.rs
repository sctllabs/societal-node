// This file is part of Substrate.

// Copyright (C) 2021-2022 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// Changes made comparing to the RuntimeOriginal tests of the FRAME pallet-collective:
// - using DAO as a parameter for pallet functions

//! DAO Collective pallet tests.

use super::{Event as CollectiveEvent, *};
use crate as pallet_dao_collective;
use dao_primitives::{
	BountyPayoutDelay, BountyUpdatePeriod, DaoToken, DispatchResultWithDaoOrigin,
	RawOrigin as DaoRawOrigin, TreasurySpendPeriod,
};
use frame_support::{
	assert_noop, assert_ok,
	instances::Instance1,
	parameter_types,
	traits::{AsEnsureOriginWithArg, ConstU32, ConstU64, EqualPrivilegeOnly, StorePreimage},
	Hashable, PalletId,
};
use frame_system::{EnsureRoot, EventRecord, Phase};
use pallet_scheduler::Event as SchedulerEvent;
use sp_core::{bounded_vec, H256};
use sp_runtime::{
	testing::Header,
	traits::{AccountIdConversion, BlakeTwo256, IdentityLookup},
	BuildStorage, Perbill,
};

pub type Block = sp_runtime::generic::Block<Header, UncheckedExtrinsic>;
pub type UncheckedExtrinsic = sp_runtime::generic::UncheckedExtrinsic<u32, u64, RuntimeCall, ()>;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: frame_system::{Pallet, Call, Event<T>},
		Collective: pallet_dao_collective::<Instance1>::{Pallet, Call, Event<T>, Origin<T>},
		CollectiveMajority: pallet_dao_collective::<Instance2>::{Pallet, Call, Event<T>, Origin<T>},
		DefaultCollective: pallet_dao_collective::{Pallet, Call, Event<T>, Origin<T>},
		Democracy: mock_democracy::{Pallet, Call, Event<T>},
		Preimage: pallet_preimage,
		Scheduler: pallet_scheduler::{Pallet, Call, Storage, Event<T>},
	}
);

mod mock_democracy {
	pub use pallet::*;
	#[frame_support::pallet]
	pub mod pallet {
		use frame_support::pallet_prelude::*;
		use frame_system::pallet_prelude::*;

		#[pallet::pallet]
		pub struct Pallet<T>(_);

		#[pallet::config]
		pub trait Config: frame_system::Config + Sized {
			type RuntimeEvent: From<Event<Self>>
				+ IsType<<Self as frame_system::Config>::RuntimeEvent>;
			type ExternalMajorityOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		}

		#[pallet::call]
		impl<T: Config> Pallet<T> {
			#[pallet::weight(0)]
			#[pallet::call_index(6)]
			pub fn external_propose_majority(origin: OriginFor<T>) -> DispatchResult {
				T::ExternalMajorityOrigin::ensure_origin(origin)?;
				Self::deposit_event(Event::<T>::ExternalProposed);
				Ok(())
			}
		}

		#[pallet::event]
		#[pallet::generate_deposit(pub(super) fn deposit_event)]
		pub enum Event<T: Config> {
			ExternalProposed,
		}
	}
}

pub type MaxMembers = ConstU32<100>;

parameter_types! {
	pub const MotionDuration: u64 = 3;
	pub const MaxProposals: u32 = 100;
	pub BlockWeights: frame_system::limits::BlockWeights =
		frame_system::limits::BlockWeights::simple_max(Weight::from_parts(1024, 0));
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
impl Config<Instance1> for Test {
	type RuntimeCall = RuntimeCall;
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type ProposalMetadataLimit = ConstU32<750>;
	type MaxProposals = MaxProposals;
	type MaxMembers = MaxMembers;
	type MaxVotes = ConstU32<100>;
	type DefaultVote = MoreThanMajorityVote;
	type WeightInfo = ();
	type DaoProvider = TestDaoProvider;
	type Preimages = Preimage;
	type Scheduler = Scheduler;
	type PalletsOrigin = OriginCaller;
}
impl Config<Instance2> for Test {
	type RuntimeCall = RuntimeCall;
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type ProposalMetadataLimit = ConstU32<750>;
	type MaxProposals = MaxProposals;
	type MaxMembers = MaxMembers;
	type MaxVotes = ConstU32<100>;
	type DefaultVote = MoreThanMajorityVote;
	type WeightInfo = ();
	type DaoProvider = TestDaoProvider;
	type Preimages = Preimage;
	type Scheduler = Scheduler;
	type PalletsOrigin = OriginCaller;
}
impl mock_democracy::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type ExternalMajorityOrigin = EnsureMembers<u64, Instance1, 1>;
}

impl pallet_preimage::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type Currency = ();
	type ManagerOrigin = EnsureRoot<u64>;
	type BaseDeposit = ConstU32<0>;
	type ByteDeposit = ConstU32<0>;
}

pub struct TestDaoProvider;
impl DaoProvider<u64, H256> for TestDaoProvider {
	type Id = u32;
	type AssetId = u128;
	type Policy = DaoPolicy;
	type Origin = RuntimeOrigin;
	type ApproveOrigin = AsEnsureOriginWithArg<frame_system::EnsureRoot<u64>>;
	type NFTCollectionId = u32;

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

	fn dao_account_id(id: Self::Id) -> u64 {
		PalletId(*b"py/sctld").into_sub_account_truncating(id)
	}

	fn dao_token(_id: Self::Id) -> Result<DaoToken<Self::AssetId, Vec<u8>>, DispatchError> {
		todo!()
	}

	fn ensure_approved(origin: Self::Origin, dao_id: Self::Id) -> DispatchResultWithDaoOrigin<u64> {
		let dao_account_id = Self::dao_account_id(dao_id);
		let approve_origin = Self::policy(dao_id)?.approve_origin;
		let dao_origin = DaoOrigin { dao_account_id, proportion: approve_origin };

		Self::ApproveOrigin::ensure_origin(origin, &dao_origin)?;

		Ok(dao_origin)
	}

	fn dao_nft_collection_id(
		_id: Self::Id,
	) -> Result<Option<Self::NFTCollectionId>, DispatchError> {
		Err(Error::<Test, Instance1>::NotMember.into())
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn create_dao(
		_founder: u64,
		_council: Vec<u64>,
		_technical_committee: Vec<u64>,
		_data: Vec<u8>,
	) -> Result<(), DispatchError> {
		Ok(())
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn approve_dao(_dao_hash: H256, _approve: bool) -> Result<(), DispatchError> {
		Ok(())
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin(dao_origin: &DaoOrigin<u64>) -> Result<Self::Origin, ()> {
		Self::ApproveOrigin::try_successful_origin(dao_origin)
	}
}

parameter_types! {
	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) * BlockWeights::get().max_block;
}

impl pallet_scheduler::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type PalletsOrigin = OriginCaller;
	type RuntimeCall = RuntimeCall;
	type MaximumWeight = ();
	type ScheduleOrigin = EnsureRoot<u64>;
	type MaxScheduledPerBlock = ConstU32<100>;
	type WeightInfo = ();
	type OriginPrivilegeCmp = EqualPrivilegeOnly;
	type Preimages = ();
}

impl From<DaoRawOrigin<u64>> for OriginCaller {
	fn from(_value: DaoRawOrigin<u64>) -> Self {
		OriginCaller::system(frame_system::RawOrigin::Root)
	}
}

impl Config for Test {
	type RuntimeCall = RuntimeCall;
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type ProposalMetadataLimit = ConstU32<750>;
	type MaxProposals = MaxProposals;
	type MaxMembers = MaxMembers;
	type MaxVotes = ConstU32<100>;
	type DefaultVote = MoreThanMajorityVote;
	type WeightInfo = ();
	type DaoProvider = TestDaoProvider;
	type Preimages = Preimage;
	type Scheduler = Scheduler;
	type PalletsOrigin = OriginCaller;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext: sp_io::TestExternalities = GenesisConfig {}.build_storage().unwrap().into();
	ext.execute_with(|| {
		System::set_block_number(1);

		init_members();
	});
	ext
}

pub(crate) fn init_members() {
	Collective::initialize_members(0, vec![1, 2, 3]).ok();
	CollectiveMajority::initialize_members(0, vec![1, 2, 3, 4, 5]).ok();
}

fn make_proposal(value: u64) -> RuntimeCall {
	RuntimeCall::System(frame_system::Call::remark_with_event {
		remark: value.to_be_bytes().to_vec(),
	})
}

fn record(event: RuntimeEvent) -> EventRecord<RuntimeEvent, H256> {
	EventRecord { phase: Phase::Initialization, event, topics: vec![] }
}

#[test]
fn motions_basic_environment_works() {
	new_test_ext().execute_with(|| {
		assert_eq!(Collective::members(0), vec![1, 2, 3]);
		assert_eq!(*Collective::proposals(0), Vec::<H256>::new());
	});
}

#[test]
fn close_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let hash = BlakeTwo256::hash_of(&proposal_bounded.unwrap());

		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));

		System::set_block_number(3);
		assert_noop!(
			Collective::close(RuntimeOrigin::signed(4), 0, hash, 0, proposal_weight, proposal_len),
			Error::<Test, Instance1>::TooEarly
		);

		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), 0, hash, 0, true));

		System::set_block_number(4);
		assert_ok!(Collective::close(
			RuntimeOrigin::signed(4),
			0,
			hash,
			0,
			proposal_weight,
			proposal_len
		));

		assert_eq!(
			System::events(),
			vec![
				record(RuntimeEvent::Scheduler(SchedulerEvent::Scheduled { when: 4, index: 0 })),
				record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
					dao_id: 0,
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					proposal,
					threshold: 1,
					meta: None,
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					voted: true,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Closed {
					dao_id: 0,
					proposal_index: 0,
					proposal_hash: hash,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Approved {
					dao_id: 0,
					proposal_index: 0,
					proposal_hash: hash
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Executed {
					dao_id: 0,
					proposal_index: 0,
					proposal_hash: hash,
					result: Err(DispatchError::BadOrigin)
				})),
				record(RuntimeEvent::Scheduler(SchedulerEvent::Canceled { when: 4, index: 0 }))
			]
		);
	});
}

#[test]
fn proposal_weight_limit_works_on_approve() {
	new_test_ext().execute_with(|| {
		let proposal = RuntimeCall::Democracy(mock_democracy::Call::external_propose_majority {});
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let hash = BlakeTwo256::hash_of(&proposal_bounded.unwrap());

		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), 0, hash, 0, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), 0, hash, 0, true));

		System::set_block_number(4);
		// TODO
		// assert_noop!(
		// 	Collective::close(
		// 		RuntimeOrigin::signed(4),
		// 		0,
		// 		hash,
		// 		0,
		// 		proposal_weight.set_ref_time(proposal_weight.ref_time() - 100),
		// 		proposal_len
		// 	),
		// 	Error::<Test, Instance1>::WrongProposalWeight
		// );
		assert_ok!(Collective::close(
			RuntimeOrigin::signed(4),
			0,
			hash,
			0,
			proposal_weight,
			proposal_len
		));
	})
}

#[test]
fn proposal_weight_limit_ignored_on_disapprove() {
	new_test_ext().execute_with(|| {
		let proposal = RuntimeCall::Democracy(mock_democracy::Call::external_propose_majority {});
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let hash = BlakeTwo256::hash_of(&proposal_bounded.unwrap());

		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		// No votes, this proposal wont pass
		System::set_block_number(4);
		assert_ok!(Collective::close(
			RuntimeOrigin::signed(4),
			0,
			hash,
			0,
			proposal_weight,
			proposal_len
		));
	})
}

#[test]
fn close_with_no_prime_but_majority_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let hash = BlakeTwo256::hash_of(&proposal_bounded.unwrap());
		assert_ok!(CollectiveMajority::initialize_members(0, vec![1, 2, 3, 4, 5]));

		assert_ok!(CollectiveMajority::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		assert_ok!(CollectiveMajority::vote(RuntimeOrigin::signed(1), 0, hash, 0, true));
		assert_ok!(CollectiveMajority::vote(RuntimeOrigin::signed(2), 0, hash, 0, true));
		assert_ok!(CollectiveMajority::vote(RuntimeOrigin::signed(3), 0, hash, 0, true));

		System::set_block_number(4);
		assert_ok!(CollectiveMajority::close(
			RuntimeOrigin::signed(4),
			0,
			hash,
			0,
			proposal_weight,
			proposal_len
		));

		assert_eq!(
			System::events(),
			vec![
				record(RuntimeEvent::Scheduler(SchedulerEvent::Scheduled { when: 4, index: 0 })),
				record(RuntimeEvent::CollectiveMajority(CollectiveEvent::Proposed {
					dao_id: 0,
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					proposal,
					threshold: 2,
					meta: None,
				})),
				record(RuntimeEvent::CollectiveMajority(CollectiveEvent::Voted {
					dao_id: 0,
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					voted: true,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::CollectiveMajority(CollectiveEvent::Voted {
					dao_id: 0,
					account: 2,
					proposal_index: 0,
					proposal_hash: hash,
					voted: true,
					yes: 2,
					no: 0
				})),
				record(RuntimeEvent::CollectiveMajority(CollectiveEvent::Voted {
					dao_id: 0,
					account: 3,
					proposal_index: 0,
					proposal_hash: hash,
					voted: true,
					yes: 3,
					no: 0
				})),
				record(RuntimeEvent::CollectiveMajority(CollectiveEvent::Closed {
					dao_id: 0,
					proposal_index: 0,
					proposal_hash: hash,
					yes: 3,
					no: 0
				})),
				record(RuntimeEvent::CollectiveMajority(CollectiveEvent::Approved {
					dao_id: 0,
					proposal_index: 0,
					proposal_hash: hash
				})),
				record(RuntimeEvent::CollectiveMajority(CollectiveEvent::Executed {
					dao_id: 0,
					proposal_index: 0,
					proposal_hash: hash,
					result: Err(DispatchError::BadOrigin)
				})),
				record(RuntimeEvent::Scheduler(SchedulerEvent::Canceled { when: 4, index: 0 }))
			]
		);
	});
}

#[test]
fn removal_of_old_voters_votes_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let hash = BlakeTwo256::hash_of(&proposal_bounded.unwrap());
		let end = 4;
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), 0, hash, 0, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), 0, hash, 0, true));
		assert_eq!(
			Collective::voting(0, &hash),
			Some(Votes {
				index: 0,
				threshold: 1,
				ayes: bounded_vec![1, 2],
				nays: bounded_vec![],
				end
			})
		);
		Collective::change_members_sorted(0, &[4], &[1], &[2, 3, 4]);
		assert_eq!(
			Collective::voting(0, &hash),
			Some(Votes {
				index: 0,
				threshold: 1,
				ayes: bounded_vec![2],
				nays: bounded_vec![],
				end
			})
		);

		let proposal = make_proposal(69);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let hash = BlakeTwo256::hash_of(&proposal_bounded.unwrap());
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(2),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), 0, hash, 1, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(3), 0, hash, 1, false));
		assert_eq!(
			Collective::voting(0, &hash),
			Some(Votes {
				index: 1,
				threshold: 1,
				ayes: bounded_vec![2],
				nays: bounded_vec![3],
				end
			})
		);
		Collective::change_members_sorted(0, &[], &[3], &[2, 4]);
		assert_eq!(
			Collective::voting(0, &hash),
			Some(Votes {
				index: 1,
				threshold: 1,
				ayes: bounded_vec![2],
				nays: bounded_vec![],
				end
			})
		);
	});
}

#[test]
fn removal_of_old_voters_votes_works_with_set_members() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let hash = BlakeTwo256::hash_of(&proposal_bounded.unwrap());
		let end = 4;
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), 0, hash, 0, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), 0, hash, 0, true));
		assert_eq!(
			Collective::voting(0, &hash),
			Some(Votes {
				index: 0,
				threshold: 1,
				ayes: bounded_vec![1, 2],
				nays: bounded_vec![],
				end
			})
		);
		assert_ok!(Collective::initialize_members(0, vec![2, 3, 4]));
		assert_eq!(
			Collective::voting(0, &hash),
			Some(Votes {
				index: 0,
				threshold: 1,
				ayes: bounded_vec![1, 2],
				nays: bounded_vec![],
				end
			})
		);

		let proposal = make_proposal(69);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let hash = BlakeTwo256::hash_of(&proposal_bounded.unwrap());
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(2),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), 0, hash, 1, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(3), 0, hash, 1, false));
		assert_eq!(
			Collective::voting(0, &hash),
			Some(Votes {
				index: 1,
				threshold: 1,
				ayes: bounded_vec![2],
				nays: bounded_vec![3],
				end
			})
		);
	});
}

#[test]
fn propose_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let hash = proposal_bounded.unwrap().blake2_256().into();
		let end = 4;
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		assert_eq!(*Collective::proposals(0), vec![hash]);
		assert_eq!(
			Collective::voting(0, &hash),
			Some(Votes { index: 0, threshold: 1, ayes: bounded_vec![], nays: bounded_vec![], end })
		);

		assert_eq!(
			System::events(),
			vec![
				record(RuntimeEvent::Scheduler(SchedulerEvent::Scheduled { when: 4, index: 0 })),
				record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
					dao_id: 0,
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					proposal,
					threshold: 1,
					meta: None,
				}))
			]
		);
	});
}

#[test]
fn limit_active_proposals() {
	new_test_ext().execute_with(|| {
		for i in 0..MaxProposals::get() {
			let proposal = make_proposal(i as u64);
			let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
			assert_ok!(Collective::propose(
				RuntimeOrigin::signed(1),
				0,
				Box::new(proposal.clone()),
				proposal_len
			));
		}
		let proposal = make_proposal(MaxProposals::get() as u64 + 1);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		assert_noop!(
			Collective::propose(
				RuntimeOrigin::signed(1),
				0,
				Box::new(proposal.clone()),
				proposal_len
			),
			Error::<Test, Instance1>::TooManyProposals
		);
	})
}

#[test]
fn correct_validate_and_get_proposal() {
	new_test_ext().execute_with(|| {
		let proposal = RuntimeCall::Democracy(mock_democracy::Call::external_propose_majority {});
		let length = proposal.encode().len() as u32;
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			length
		));

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let hash = BlakeTwo256::hash_of(&proposal_bounded.unwrap());
		let weight = proposal.get_dispatch_info().weight;
		assert_noop!(
			Collective::validate_and_get_proposal(
				0,
				&BlakeTwo256::hash_of(&vec![3; 4]),
				length,
				weight
			),
			Error::<Test, Instance1>::ProposalMissing
		);
		// TODO
		// assert_noop!(
		// 	Collective::validate_and_get_proposal(0, &hash, length - 2, weight),
		// 	Error::<Test, Instance1>::WrongProposalLength
		// );
		// TODO
		// assert_noop!(
		// 	Collective::validate_and_get_proposal(
		// 		0,
		// 		&hash,
		// 		length,
		// 		weight.set_ref_time(weight.ref_time() - 100)
		// 	),
		// 	Error::<Test, Instance1>::WrongProposalWeight
		// );
		let res = Collective::validate_and_get_proposal(0, &hash, length, weight);
		assert_ok!(res.clone());
		let (retrieved_proposal, _len) = res.unwrap();
		// assert_eq!(length as usize, len);
		assert_eq!(proposal, retrieved_proposal);
	})
}

#[test]
fn motions_ignoring_non_collective_proposals_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		assert_noop!(
			Collective::propose(
				RuntimeOrigin::signed(42),
				0,
				Box::new(proposal.clone()),
				proposal_len
			),
			Error::<Test, Instance1>::NotMember
		);
	});
}

#[test]
fn motions_ignoring_non_collective_votes_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let hash: H256 = proposal_bounded.unwrap().blake2_256().into();
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		assert_noop!(
			Collective::vote(RuntimeOrigin::signed(42), 0, hash, 0, true),
			Error::<Test, Instance1>::NotMember,
		);
	});
}

#[test]
fn motions_ignoring_bad_index_collective_vote_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(3);
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let hash: H256 = proposal_bounded.unwrap().blake2_256().into();
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		assert_noop!(
			Collective::vote(RuntimeOrigin::signed(2), 0, hash, 1, true),
			Error::<Test, Instance1>::WrongIndex,
		);
	});
}

#[test]
fn motions_vote_after_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let hash: H256 = proposal_bounded.unwrap().blake2_256().into();
		let end = 4;
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		// Initially there a no votes when the motion is proposed.
		assert_eq!(
			Collective::voting(0, &hash),
			Some(Votes { index: 0, threshold: 1, ayes: bounded_vec![], nays: bounded_vec![], end })
		);
		// Cast first aye vote.
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), 0, hash, 0, true));
		assert_eq!(
			Collective::voting(0, &hash),
			Some(Votes {
				index: 0,
				threshold: 1,
				ayes: bounded_vec![1],
				nays: bounded_vec![],
				end
			})
		);
		// Try to cast a duplicate aye vote.
		assert_noop!(
			Collective::vote(RuntimeOrigin::signed(1), 0, hash, 0, true),
			Error::<Test, Instance1>::DuplicateVote,
		);
		// Cast a nay vote.
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), 0, hash, 0, false));
		assert_eq!(
			Collective::voting(0, &hash),
			Some(Votes {
				index: 0,
				threshold: 1,
				ayes: bounded_vec![],
				nays: bounded_vec![1],
				end
			})
		);
		// Try to cast a duplicate nay vote.
		assert_noop!(
			Collective::vote(RuntimeOrigin::signed(1), 0, hash, 0, false),
			Error::<Test, Instance1>::DuplicateVote,
		);

		assert_eq!(
			System::events(),
			vec![
				record(RuntimeEvent::Scheduler(SchedulerEvent::Scheduled { when: 4, index: 0 })),
				record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
					dao_id: 0,
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					proposal,
					threshold: 1,
					meta: None,
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					voted: true,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					voted: false,
					yes: 0,
					no: 1
				})),
			]
		);
	});
}

#[test]
fn motions_reproposing_disapproved_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let hash: H256 = proposal_bounded.unwrap().blake2_256().into();
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), 0, hash, 0, false));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(3), 0, hash, 0, false));
		assert_ok!(Collective::close(
			RuntimeOrigin::signed(2),
			0,
			hash,
			0,
			proposal_weight,
			proposal_len
		));
		assert_eq!(*Collective::proposals(0), vec![]);
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		assert_eq!(*Collective::proposals(0), vec![hash]);
	});
}

#[test]
fn motions_approval_with_enough_votes_and_lower_voting_threshold_works() {
	new_test_ext().execute_with(|| {
		let proposal = RuntimeCall::Democracy(mock_democracy::Call::external_propose_majority {});
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let hash: H256 = proposal_bounded.unwrap().blake2_256().into();
		// The voting threshold is 2, but the required votes for `ExternalMajorityOrigin` is 3.
		// The proposal will be executed regardless of the voting threshold
		// as long as we have enough yes votes.
		//
		// Failed to execute with only 2 yes votes.
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), 0, hash, 0, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), 0, hash, 0, true));
		assert_ok!(Collective::close(
			RuntimeOrigin::signed(2),
			0,
			hash,
			0,
			proposal_weight,
			proposal_len
		));
		assert_eq!(
			System::events(),
			vec![
				record(RuntimeEvent::Scheduler(SchedulerEvent::Scheduled { when: 4, index: 0 })),
				record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
					dao_id: 0,
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					proposal: proposal.clone(),
					threshold: 1,
					meta: None,
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					voted: true,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 2,
					proposal_index: 0,
					proposal_hash: hash,
					voted: true,
					yes: 2,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Closed {
					dao_id: 0,
					proposal_index: 0,
					proposal_hash: hash,
					yes: 2,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Approved {
					dao_id: 0,
					proposal_index: 0,
					proposal_hash: hash
				})),
				record(RuntimeEvent::Democracy(
					mock_democracy::pallet::Event::<Test>::ExternalProposed
				)),
				record(RuntimeEvent::Collective(CollectiveEvent::Executed {
					dao_id: 0,
					proposal_index: 0,
					proposal_hash: hash,
					result: Ok(())
				})),
				record(RuntimeEvent::Scheduler(SchedulerEvent::Canceled { when: 4, index: 0 }))
			]
		);

		System::reset_events();

		// Executed with 3 yes votes.
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), 0, hash, 1, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), 0, hash, 1, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(3), 0, hash, 1, true));
		assert_ok!(Collective::close(
			RuntimeOrigin::signed(2),
			0,
			hash,
			1,
			proposal_weight,
			proposal_len
		));
		assert_eq!(
			System::events(),
			vec![
				record(RuntimeEvent::Scheduler(SchedulerEvent::Scheduled { when: 4, index: 0 })),
				record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
					dao_id: 0,
					account: 1,
					proposal_index: 1,
					proposal_hash: hash,
					proposal,
					threshold: 1,
					meta: None,
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 1,
					proposal_index: 1,
					proposal_hash: hash,
					voted: true,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 2,
					proposal_index: 1,
					proposal_hash: hash,
					voted: true,
					yes: 2,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 3,
					proposal_index: 1,
					proposal_hash: hash,
					voted: true,
					yes: 3,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Closed {
					dao_id: 0,
					proposal_index: 1,
					proposal_hash: hash,
					yes: 3,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Approved {
					dao_id: 0,
					proposal_index: 1,
					proposal_hash: hash
				})),
				record(RuntimeEvent::Democracy(
					mock_democracy::pallet::Event::<Test>::ExternalProposed
				)),
				record(RuntimeEvent::Collective(CollectiveEvent::Executed {
					dao_id: 0,
					proposal_index: 1,
					proposal_hash: hash,
					result: Ok(())
				})),
				record(RuntimeEvent::Scheduler(SchedulerEvent::Canceled { when: 4, index: 0 }))
			]
		);
	});
}

#[test]
fn motions_disapproval_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let hash: H256 = proposal_bounded.unwrap().blake2_256().into();
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), 0, hash, 0, false));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(3), 0, hash, 0, false));
		assert_ok!(Collective::close(
			RuntimeOrigin::signed(2),
			0,
			hash,
			0,
			proposal_weight,
			proposal_len
		));

		assert_eq!(
			System::events(),
			vec![
				record(RuntimeEvent::Scheduler(SchedulerEvent::Scheduled { when: 4, index: 0 })),
				record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
					dao_id: 0,
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					proposal,
					threshold: 1,
					meta: None,
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 2,
					proposal_index: 0,
					proposal_hash: hash,
					voted: false,
					yes: 0,
					no: 1
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 3,
					proposal_index: 0,
					proposal_hash: hash,
					voted: false,
					yes: 0,
					no: 2
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Closed {
					dao_id: 0,
					proposal_index: 0,
					proposal_hash: hash,
					yes: 0,
					no: 2
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Disapproved {
					dao_id: 0,
					proposal_index: 0,
					proposal_hash: hash
				})),
				record(RuntimeEvent::Scheduler(SchedulerEvent::Canceled { when: 4, index: 0 }))
			]
		);
	});
}

#[test]
fn motions_approval_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let hash: H256 = proposal_bounded.unwrap().blake2_256().into();
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), 0, hash, 0, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), 0, hash, 0, true));
		assert_ok!(Collective::close(
			RuntimeOrigin::signed(2),
			0,
			hash,
			0,
			proposal_weight,
			proposal_len
		));

		assert_eq!(
			System::events(),
			vec![
				record(RuntimeEvent::Scheduler(SchedulerEvent::Scheduled { when: 4, index: 0 })),
				record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
					dao_id: 0,
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					proposal,
					threshold: 1,
					meta: None,
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					voted: true,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 2,
					proposal_index: 0,
					proposal_hash: hash,
					voted: true,
					yes: 2,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Closed {
					dao_id: 0,
					proposal_index: 0,
					proposal_hash: hash,
					yes: 2,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Approved {
					dao_id: 0,
					proposal_index: 0,
					proposal_hash: hash
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Executed {
					dao_id: 0,
					proposal_index: 0,
					proposal_hash: hash,
					result: Err(DispatchError::BadOrigin)
				})),
				record(RuntimeEvent::Scheduler(SchedulerEvent::Canceled { when: 4, index: 0 }))
			]
		);
	});
}

#[test]
fn motion_with_no_votes_closes_with_disapproval() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let hash: H256 = proposal_bounded.unwrap().blake2_256().into();
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		assert_eq!(
			System::events()[1],
			record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
				dao_id: 0,
				account: 1,
				proposal_index: 0,
				proposal_hash: hash,
				proposal,
				threshold: 1,
				meta: None,
			}))
		);

		// Closing the motion too early is not possible because it has neither
		// an approving or disapproving simple majority due to the lack of votes.
		assert_noop!(
			Collective::close(RuntimeOrigin::signed(2), 0, hash, 0, proposal_weight, proposal_len),
			Error::<Test, Instance1>::TooEarly
		);

		// Once the motion duration passes,
		let closing_block = System::block_number() +
			<Test as Config>::DaoProvider::policy(0).ok().unwrap().proposal_period as u64;
		System::set_block_number(closing_block);
		// we can successfully close the motion.
		assert_ok!(Collective::close(
			RuntimeOrigin::signed(2),
			0,
			hash,
			0,
			proposal_weight,
			proposal_len
		));

		// Events show that the close ended in a disapproval.
		assert_eq!(
			System::events()[2],
			record(RuntimeEvent::Collective(CollectiveEvent::Closed {
				dao_id: 0,
				proposal_index: 0,
				proposal_hash: hash,
				yes: 0,
				no: 3
			}))
		);
		assert_eq!(
			System::events()[3],
			record(RuntimeEvent::Collective(CollectiveEvent::Disapproved {
				dao_id: 0,
				proposal_index: 0,
				proposal_hash: hash
			}))
		);
	})
}

#[test]
fn close_disapprove_does_not_care_about_weight_or_len() {
	// This test confirms that if you close a proposal that would be disapproved,
	// we do not care about the proposal length or proposal weight since it will
	// not be read from storage or executed.
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let hash: H256 = proposal_bounded.unwrap().blake2_256().into();
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		// First we make the proposal succeed
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), 0, hash, 0, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), 0, hash, 0, true));
		// It will not close with bad weight/len information
		// assert_noop!(
		// 	Collective::close(RuntimeOrigin::signed(2), 0, hash, 0, Weight::zero(), 0),
		// 	Error::<Test, Instance1>::WrongProposalLength,
		// );
		// TODO
		// assert_noop!(
		// 	Collective::close(RuntimeOrigin::signed(2), 0, hash, 0, Weight::zero(), proposal_len),
		// 	Error::<Test, Instance1>::WrongProposalWeight,
		// );
		// Now we make the proposal fail
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), 0, hash, 0, false));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), 0, hash, 0, false));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(3), 0, hash, 0, false));
		// It can close even if the weight/len information is bad
		assert_ok!(Collective::close(RuntimeOrigin::signed(2), 0, hash, 0, Weight::zero(), 0));
	})
}
