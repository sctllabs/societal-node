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
use dao_primitives::{AccountTokenBalance, DaoToken};
use frame_support::{
	assert_noop, assert_ok, parameter_types,
	traits::{ConstU32, ConstU64},
	weights::Pays,
	Hashable, PalletId,
};
use frame_system::{EventRecord, Phase};
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{AccountIdConversion, BlakeTwo256, IdentityLookup},
	BuildStorage,
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
	}
);

mod mock_democracy {
	pub use pallet::*;
	#[frame_support::pallet]
	pub mod pallet {
		use frame_support::pallet_prelude::*;
		use frame_system::pallet_prelude::*;

		#[pallet::pallet]
		#[pallet::generate_store(pub(super) trait Store)]
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
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type ProposalMetadataLimit = ConstU32<100>;
	type MaxProposals = MaxProposals;
	type MaxMembers = MaxMembers;
	type DefaultVote = MoreThanMajorityVote;
	type WeightInfo = ();
	type DaoProvider = TestDaoProvider;
}
impl Config<Instance2> for Test {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type ProposalMetadataLimit = ConstU32<100>;
	type MaxProposals = MaxProposals;
	type MaxMembers = MaxMembers;
	type DefaultVote = MoreThanMajorityVote;
	type WeightInfo = ();
	type DaoProvider = TestDaoProvider;
}
impl mock_democracy::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type ExternalMajorityOrigin = EnsureMembers<u64, Instance1, 1>;
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
			proposal_period: 3,
			approve_origin: DaoPolicyProportion::AtLeast((1, 2)),
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
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type ProposalMetadataLimit = ConstU32<100>;
	type MaxProposals = MaxProposals;
	type MaxMembers = MaxMembers;
	type DefaultVote = MoreThanMajorityVote;
	type WeightInfo = ();
	type DaoProvider = TestDaoProvider;
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
	Collective::set_members(RuntimeOrigin::root(), 0, vec![1, 2, 3], 0).ok();
	CollectiveMajority::set_members(RuntimeOrigin::root(), 0, vec![1, 2, 3, 4, 5], 0).ok();
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
		let hash = BlakeTwo256::hash_of(&proposal);

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
				record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
					dao_id: 0,
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					threshold: 1,
					meta: None,
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 1,
					proposal_hash: hash,
					voted: true,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Closed {
					dao_id: 0,
					proposal_hash: hash,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Approved {
					dao_id: 0,
					proposal_hash: hash
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Executed {
					dao_id: 0,
					proposal_hash: hash,
					result: Err(DispatchError::BadOrigin)
				}))
			]
		);
	});
}

#[test]
fn proposal_weight_limit_works_on_approve() {
	new_test_ext().execute_with(|| {
		let proposal = RuntimeCall::Collective(crate::Call::set_members {
			dao_id: 0,
			new_members: vec![1, 2, 3],
			old_count: MaxMembers::get(),
		});
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;
		let hash = BlakeTwo256::hash_of(&proposal);

		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), 0, hash, 0, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), 0, hash, 0, true));

		System::set_block_number(4);
		assert_noop!(
			Collective::close(
				RuntimeOrigin::signed(4),
				0,
				hash,
				0,
				proposal_weight.set_ref_time(proposal_weight.ref_time() - 100),
				proposal_len
			),
			Error::<Test, Instance1>::WrongProposalWeight
		);
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
		let proposal = RuntimeCall::Collective(crate::Call::set_members {
			dao_id: 0,
			new_members: vec![1, 2, 3],
			old_count: MaxMembers::get(),
		});
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;
		let hash = BlakeTwo256::hash_of(&proposal);

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
		let hash = BlakeTwo256::hash_of(&proposal);
		assert_ok!(CollectiveMajority::set_members(
			RuntimeOrigin::root(),
			0,
			vec![1, 2, 3, 4, 5],
			MaxMembers::get()
		));

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
				record(RuntimeEvent::CollectiveMajority(CollectiveEvent::Proposed {
					dao_id: 0,
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					threshold: 2,
					meta: None,
				})),
				record(RuntimeEvent::CollectiveMajority(CollectiveEvent::Voted {
					dao_id: 0,
					account: 1,
					proposal_hash: hash,
					voted: true,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::CollectiveMajority(CollectiveEvent::Voted {
					dao_id: 0,
					account: 2,
					proposal_hash: hash,
					voted: true,
					yes: 2,
					no: 0
				})),
				record(RuntimeEvent::CollectiveMajority(CollectiveEvent::Voted {
					dao_id: 0,
					account: 3,
					proposal_hash: hash,
					voted: true,
					yes: 3,
					no: 0
				})),
				record(RuntimeEvent::CollectiveMajority(CollectiveEvent::Closed {
					dao_id: 0,
					proposal_hash: hash,
					yes: 3,
					no: 0
				})),
				record(RuntimeEvent::CollectiveMajority(CollectiveEvent::Approved {
					dao_id: 0,
					proposal_hash: hash
				})),
				record(RuntimeEvent::CollectiveMajority(CollectiveEvent::Executed {
					dao_id: 0,
					proposal_hash: hash,
					result: Err(DispatchError::BadOrigin)
				}))
			]
		);
	});
}

#[test]
fn removal_of_old_voters_votes_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash = BlakeTwo256::hash_of(&proposal);
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
			Some(Votes { index: 0, threshold: 1, ayes: vec![1, 2], nays: vec![], end })
		);
		Collective::change_members_sorted(0, &[4], &[1], &[2, 3, 4]);
		assert_eq!(
			Collective::voting(0, &hash),
			Some(Votes { index: 0, threshold: 1, ayes: vec![2], nays: vec![], end })
		);

		let proposal = make_proposal(69);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash = BlakeTwo256::hash_of(&proposal);
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
			Some(Votes { index: 1, threshold: 1, ayes: vec![2], nays: vec![3], end })
		);
		Collective::change_members_sorted(0, &[], &[3], &[2, 4]);
		assert_eq!(
			Collective::voting(0, &hash),
			Some(Votes { index: 1, threshold: 1, ayes: vec![2], nays: vec![], end })
		);
	});
}

#[test]
fn removal_of_old_voters_votes_works_with_set_members() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash = BlakeTwo256::hash_of(&proposal);
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
			Some(Votes { index: 0, threshold: 1, ayes: vec![1, 2], nays: vec![], end })
		);
		assert_ok!(Collective::set_members(
			RuntimeOrigin::root(),
			0,
			vec![2, 3, 4],
			MaxMembers::get()
		));
		assert_eq!(
			Collective::voting(0, &hash),
			Some(Votes { index: 0, threshold: 1, ayes: vec![2], nays: vec![], end })
		);

		let proposal = make_proposal(69);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash = BlakeTwo256::hash_of(&proposal);
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
			Some(Votes { index: 1, threshold: 1, ayes: vec![2], nays: vec![3], end })
		);
		assert_ok!(Collective::set_members(
			RuntimeOrigin::root(),
			0,
			vec![2, 4],
			MaxMembers::get()
		));
		assert_eq!(
			Collective::voting(0, &hash),
			Some(Votes { index: 1, threshold: 1, ayes: vec![2], nays: vec![], end })
		);
	});
}

#[test]
fn propose_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash = proposal.blake2_256().into();
		let end = 4;
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		assert_eq!(*Collective::proposals(0), vec![hash]);
		assert_eq!(Collective::proposal_of(0, &hash), Some(proposal));
		assert_eq!(
			Collective::voting(0, &hash),
			Some(Votes { index: 0, threshold: 1, ayes: vec![], nays: vec![], end })
		);

		assert_eq!(
			System::events(),
			vec![record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
				dao_id: 0,
				account: 1,
				proposal_index: 0,
				proposal_hash: hash,
				threshold: 1,
				meta: None,
			}))]
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
		let proposal = RuntimeCall::Collective(crate::Call::set_members {
			dao_id: 0,
			new_members: vec![1, 2, 3],
			old_count: MaxMembers::get(),
		});
		let length = proposal.encode().len() as u32;
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			length
		));

		let hash = BlakeTwo256::hash_of(&proposal);
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
		assert_noop!(
			Collective::validate_and_get_proposal(0, &hash, length - 2, weight),
			Error::<Test, Instance1>::WrongProposalLength
		);

		assert_noop!(
			Collective::validate_and_get_proposal(
				0,
				&hash,
				length,
				weight.set_ref_time(weight.ref_time() - 100)
			),
			Error::<Test, Instance1>::WrongProposalWeight
		);
		let res = Collective::validate_and_get_proposal(0, &hash, length, weight);
		assert_ok!(res.clone());
		let (retrieved_proposal, len) = res.unwrap();
		assert_eq!(length as usize, len);
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
		let hash: H256 = proposal.blake2_256().into();
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
		let hash: H256 = proposal.blake2_256().into();
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
		let hash: H256 = proposal.blake2_256().into();
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
			Some(Votes { index: 0, threshold: 1, ayes: vec![], nays: vec![], end })
		);
		// Cast first aye vote.
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), 0, hash, 0, true));
		assert_eq!(
			Collective::voting(0, &hash),
			Some(Votes { index: 0, threshold: 1, ayes: vec![1], nays: vec![], end })
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
			Some(Votes { index: 0, threshold: 1, ayes: vec![], nays: vec![1], end })
		);
		// Try to cast a duplicate nay vote.
		assert_noop!(
			Collective::vote(RuntimeOrigin::signed(1), 0, hash, 0, false),
			Error::<Test, Instance1>::DuplicateVote,
		);

		assert_eq!(
			System::events(),
			vec![
				record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
					dao_id: 0,
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					threshold: 1,
					meta: None,
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 1,
					proposal_hash: hash,
					voted: true,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 1,
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
fn motions_all_first_vote_free_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash: H256 = proposal.blake2_256().into();
		let end = 4;
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len,
		));
		assert_eq!(
			Collective::voting(0, &hash),
			Some(Votes { index: 0, threshold: 1, ayes: vec![], nays: vec![], end })
		);

		// For the motion, acc 2's first vote, expecting Ok with Pays::No.
		let vote_rval: DispatchResultWithPostInfo =
			Collective::vote(RuntimeOrigin::signed(2), 0, hash, 0, true);
		assert_eq!(vote_rval.unwrap().pays_fee, Pays::No);

		// Duplicate vote, expecting error with Pays::Yes.
		let vote_rval: DispatchResultWithPostInfo =
			Collective::vote(RuntimeOrigin::signed(2), 0, hash, 0, true);
		assert_eq!(vote_rval.unwrap_err().post_info.pays_fee, Pays::Yes);

		// Modifying vote, expecting ok with Pays::Yes.
		let vote_rval: DispatchResultWithPostInfo =
			Collective::vote(RuntimeOrigin::signed(2), 0, hash, 0, false);
		assert_eq!(vote_rval.unwrap().pays_fee, Pays::Yes);

		// For the motion, acc 3's first vote, expecting Ok with Pays::No.
		let vote_rval: DispatchResultWithPostInfo =
			Collective::vote(RuntimeOrigin::signed(3), 0, hash, 0, true);
		assert_eq!(vote_rval.unwrap().pays_fee, Pays::No);

		// acc 3 modify the vote, expecting Ok with Pays::Yes.
		let vote_rval: DispatchResultWithPostInfo =
			Collective::vote(RuntimeOrigin::signed(3), 0, hash, 0, false);
		assert_eq!(vote_rval.unwrap().pays_fee, Pays::Yes);

		// Test close() Extrincis | Check DispatchResultWithPostInfo with Pay Info

		let proposal_weight = proposal.get_dispatch_info().weight;
		let close_rval: DispatchResultWithPostInfo =
			Collective::close(RuntimeOrigin::signed(2), 0, hash, 0, proposal_weight, proposal_len);
		assert_eq!(close_rval.unwrap().pays_fee, Pays::No);

		// trying to close the proposal, which is already closed.
		// Expecting error "ProposalMissing" with Pays::Yes
		let close_rval: DispatchResultWithPostInfo =
			Collective::close(RuntimeOrigin::signed(2), 0, hash, 0, proposal_weight, proposal_len);
		assert_eq!(close_rval.unwrap_err().post_info.pays_fee, Pays::Yes);
	});
}

#[test]
fn motions_reproposing_disapproved_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let proposal_weight = proposal.get_dispatch_info().weight;
		let hash: H256 = proposal.blake2_256().into();
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
		let hash: H256 = proposal.blake2_256().into();
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
				record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
					dao_id: 0,
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					threshold: 1,
					meta: None,
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 1,
					proposal_hash: hash,
					voted: true,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 2,
					proposal_hash: hash,
					voted: true,
					yes: 2,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Closed {
					dao_id: 0,
					proposal_hash: hash,
					yes: 2,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Approved {
					dao_id: 0,
					proposal_hash: hash
				})),
				record(RuntimeEvent::Democracy(
					mock_democracy::pallet::Event::<Test>::ExternalProposed
				)),
				record(RuntimeEvent::Collective(CollectiveEvent::Executed {
					dao_id: 0,
					proposal_hash: hash,
					result: Ok(())
				})),
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
				record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
					dao_id: 0,
					account: 1,
					proposal_index: 1,
					proposal_hash: hash,
					threshold: 1,
					meta: None,
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 1,
					proposal_hash: hash,
					voted: true,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 2,
					proposal_hash: hash,
					voted: true,
					yes: 2,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 3,
					proposal_hash: hash,
					voted: true,
					yes: 3,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Closed {
					dao_id: 0,
					proposal_hash: hash,
					yes: 3,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Approved {
					dao_id: 0,
					proposal_hash: hash
				})),
				record(RuntimeEvent::Democracy(
					mock_democracy::pallet::Event::<Test>::ExternalProposed
				)),
				record(RuntimeEvent::Collective(CollectiveEvent::Executed {
					dao_id: 0,
					proposal_hash: hash,
					result: Ok(())
				})),
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
		let hash: H256 = proposal.blake2_256().into();
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
				record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
					dao_id: 0,
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					threshold: 1,
					meta: None,
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 2,
					proposal_hash: hash,
					voted: false,
					yes: 0,
					no: 1
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 3,
					proposal_hash: hash,
					voted: false,
					yes: 0,
					no: 2
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Closed {
					dao_id: 0,
					proposal_hash: hash,
					yes: 0,
					no: 2
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Disapproved {
					dao_id: 0,
					proposal_hash: hash
				})),
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
		let hash: H256 = proposal.blake2_256().into();
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
				record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
					dao_id: 0,
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					threshold: 1,
					meta: None,
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 1,
					proposal_hash: hash,
					voted: true,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 2,
					proposal_hash: hash,
					voted: true,
					yes: 2,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Closed {
					dao_id: 0,
					proposal_hash: hash,
					yes: 2,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Approved {
					dao_id: 0,
					proposal_hash: hash
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Executed {
					dao_id: 0,
					proposal_hash: hash,
					result: Err(DispatchError::BadOrigin)
				})),
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
		let hash: H256 = proposal.blake2_256().into();
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		assert_eq!(
			System::events()[0],
			record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
				dao_id: 0,
				account: 1,
				proposal_index: 0,
				proposal_hash: hash,
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
			System::events()[1],
			record(RuntimeEvent::Collective(CollectiveEvent::Closed {
				dao_id: 0,
				proposal_hash: hash,
				yes: 0,
				no: 3
			}))
		);
		assert_eq!(
			System::events()[2],
			record(RuntimeEvent::Collective(CollectiveEvent::Disapproved {
				dao_id: 0,
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
		let hash: H256 = proposal.blake2_256().into();
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
		assert_noop!(
			Collective::close(RuntimeOrigin::signed(2), 0, hash, 0, Weight::zero(), 0),
			Error::<Test, Instance1>::WrongProposalLength,
		);
		assert_noop!(
			Collective::close(RuntimeOrigin::signed(2), 0, hash, 0, Weight::zero(), proposal_len),
			Error::<Test, Instance1>::WrongProposalWeight,
		);
		// Now we make the proposal fail
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), 0, hash, 0, false));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), 0, hash, 0, false));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(3), 0, hash, 0, false));
		// It can close even if the weight/len information is bad
		assert_ok!(Collective::close(RuntimeOrigin::signed(2), 0, hash, 0, Weight::zero(), 0));
	})
}

#[test]
fn disapprove_proposal_works() {
	new_test_ext().execute_with(|| {
		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let hash: H256 = proposal.blake2_256().into();
		assert_ok!(Collective::propose(
			RuntimeOrigin::signed(1),
			0,
			Box::new(proposal.clone()),
			proposal_len
		));
		// Proposal would normally succeed
		assert_ok!(Collective::vote(RuntimeOrigin::signed(1), 0, hash, 0, true));
		assert_ok!(Collective::vote(RuntimeOrigin::signed(2), 0, hash, 0, true));
		// But Root can disapprove and remove it anyway
		assert_ok!(Collective::disapprove_proposal(RuntimeOrigin::root(), 0, hash));
		assert_eq!(
			System::events(),
			vec![
				record(RuntimeEvent::Collective(CollectiveEvent::Proposed {
					dao_id: 0,
					account: 1,
					proposal_index: 0,
					proposal_hash: hash,
					threshold: 1,
					meta: None,
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 1,
					proposal_hash: hash,
					voted: true,
					yes: 1,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Voted {
					dao_id: 0,
					account: 2,
					proposal_hash: hash,
					voted: true,
					yes: 2,
					no: 0
				})),
				record(RuntimeEvent::Collective(CollectiveEvent::Disapproved {
					dao_id: 0,
					proposal_hash: hash
				})),
			]
		);
	})
}
