use crate::{mock::*, BoundedVec, Config, Error, Vote};
use dao_primitives::{PendingProposal, PendingVote};
use frame_support::{
	assert_noop, assert_ok,
	codec::Encode,
	dispatch::RawOrigin,
	traits::{Bounded, StorePreimage},
	weights::Weight,
};
use sp_core::{crypto::Ss58Codec, sr25519::Public, H256};
use sp_runtime::{
	traits::{BlakeTwo256, Hash},
	DispatchResult,
};
use sp_std::str::FromStr;

fn make_proposal(value: u64) -> RuntimeCall {
	RuntimeCall::System(frame_system::Call::remark_with_event {
		remark: value.to_be_bytes().to_vec(),
	})
}

fn get_dao() -> Vec<u8> {
	serde_json::to_vec(&get_dao_json()).ok().unwrap()
}

fn create_dao(who: Public) -> DispatchResult {
	DaoFactory::create_dao(RuntimeOrigin::signed(who), vec![], vec![], get_dao())
}

fn approve_dao(hash: H256) -> DispatchResult {
	DaoFactory::approve_dao(RawOrigin::None.into(), hash, true)
}

// TODO: add offchain worker

#[test]
fn propose_account_invalid() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let alice_eth = "alice".to_string();

		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);

		assert_noop!(
			DaoEthGovernance::propose(
				RuntimeOrigin::signed(alice),
				0,
				Box::new(proposal),
				proposal_len,
				alice_eth.into()
			),
			Error::<Test>::InvalidInput
		);
	})
}

#[test]
#[cfg(not(feature = "runtime-benchmarks"))]
fn propose_account_not_signer() {
	new_test_ext().execute_with(|| {
		let alice = Public::from_string("/Alice").ok().unwrap();
		let gerald_eth = "0x6Be02d1d3665660d22FF9624b7BE0551ee1Ac91b".to_string();

		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);

		assert_noop!(
			DaoEthGovernance::propose(
				RuntimeOrigin::signed(alice),
				0,
				Box::new(proposal),
				proposal_len,
				gerald_eth.into()
			),
			Error::<Test>::WrongAccountId
		);
	})
}

#[test]
fn propose_pending_works() {
	new_test_ext().execute_with(|| {
		let gerald = Public::from_str("5CNJv1vQjABY9W3BtsV2tzaLCjZepWXaYYzuDGWUUNVvMjcG")
			.ok()
			.unwrap();
		let gerald_eth = "0x6Be02d1d3665660d22FF9624b7BE0551ee1Ac91b".to_string();

		assert_ok!(create_dao(gerald));
		assert_ok!(approve_dao(
			H256::from_str("0x1a701f5720b125c5947bd7de0869d321785460533f4220c44b7cb1f10e91b0dd")
				.ok()
				.unwrap()
		));

		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let hash =
			H256::from_str("0xa97bb58cb53450f544cb331acc5d8844846c1b80456699a9939a5bf1bb1057d9")
				.ok()
				.unwrap();

		assert_ok!(DaoEthGovernance::propose(
			RuntimeOrigin::signed(gerald.clone()),
			0,
			Box::new(proposal.clone()),
			proposal_len,
			gerald_eth.into()
		));

		let pending_proposal = DaoEthGovernance::pending_proposal_of(0, hash).unwrap();

		assert_eq!(
			pending_proposal,
			(
				PendingProposal {
					who: gerald,
					length_bound: 11,
					meta: BoundedVec::<u8, <Test as Config>::ProposalMetadataLimit>::try_from(
						vec![]
					)
					.unwrap(),
					block_number: 0,
				},
				proposal_bounded.unwrap()
			)
		);
	})
}

#[test]
fn propose_approve_works() {
	new_test_ext().execute_with(|| {
		let gerald = Public::from_str("5CNJv1vQjABY9W3BtsV2tzaLCjZepWXaYYzuDGWUUNVvMjcG")
			.ok()
			.unwrap();
		let gerald_eth = "0x6Be02d1d3665660d22FF9624b7BE0551ee1Ac91b".to_string();

		assert_ok!(create_dao(gerald));
		assert_ok!(approve_dao(
			H256::from_str("0x1a701f5720b125c5947bd7de0869d321785460533f4220c44b7cb1f10e91b0dd")
				.ok()
				.unwrap()
		));

		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let hash =
			H256::from_str("0xa97bb58cb53450f544cb331acc5d8844846c1b80456699a9939a5bf1bb1057d9")
				.ok()
				.unwrap();

		assert_ok!(DaoEthGovernance::propose(
			RuntimeOrigin::signed(gerald.clone()),
			0,
			Box::new(proposal.clone()),
			proposal_len,
			gerald_eth.into()
		));

		assert_ok!(DaoEthGovernance::approve_propose(
			RawOrigin::None.into(),
			0,
			100_u128,
			0,
			hash,
			true
		));

		assert_eq!(DaoEthGovernance::pending_proposal_of(0, hash), None);

		let bounded_hash =
			H256::from_str("0x5351449fdb96b2e521ad3564da1f023266612394357a8c87be08f25a37cec7a8")
				.ok()
				.unwrap();
		assert_eq!(
			DaoEthGovernance::proposal_of(0, bounded_hash).unwrap(),
			proposal_bounded.unwrap()
		);
	})
}

#[test]
fn vote_pending_works() {
	new_test_ext().execute_with(|| {
		let gerald = Public::from_str("5CNJv1vQjABY9W3BtsV2tzaLCjZepWXaYYzuDGWUUNVvMjcG")
			.ok()
			.unwrap();
		let gerald_eth = "0x6Be02d1d3665660d22FF9624b7BE0551ee1Ac91b".to_string();

		assert_ok!(create_dao(gerald));
		assert_ok!(approve_dao(
			H256::from_str("0x1a701f5720b125c5947bd7de0869d321785460533f4220c44b7cb1f10e91b0dd")
				.ok()
				.unwrap()
		));

		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);

		let pending_hash =
			H256::from_str("0xa97bb58cb53450f544cb331acc5d8844846c1b80456699a9939a5bf1bb1057d9")
				.ok()
				.unwrap();

		assert_ok!(DaoEthGovernance::propose(
			RuntimeOrigin::signed(gerald.clone()),
			0,
			Box::new(proposal.clone()),
			proposal_len,
			gerald_eth.clone().into()
		));

		assert_ok!(DaoEthGovernance::approve_propose(
			RawOrigin::None.into(),
			0,
			10,
			0,
			pending_hash.clone(),
			true
		));

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let proposal_hash = BlakeTwo256::hash_of(&proposal_bounded.unwrap());

		assert_ok!(DaoEthGovernance::vote(
			RuntimeOrigin::signed(gerald.clone()),
			0,
			proposal_hash,
			0,
			Vote { aye: true, balance: 10_u32.into() },
			gerald_eth.into()
		));

		let pending_vote_hash =
			H256::from_str("0xfd6540af73c6a68ab53b67cefd77c779eca8941f255cb03498ac2280424e4cc7")
				.ok()
				.unwrap();
		let pending_vote = DaoEthGovernance::pending_voting(0, pending_vote_hash).unwrap();
		assert_eq!(
			pending_vote,
			PendingVote {
				who: gerald,
				proposal_hash,
				proposal_index: 0,
				aye: true,
				balance: 10_u32.into(),
				block_number: 0,
			}
		);
	})
}

#[test]
fn vote_approve_works() {
	new_test_ext().execute_with(|| {
		let gerald = Public::from_str("5CNJv1vQjABY9W3BtsV2tzaLCjZepWXaYYzuDGWUUNVvMjcG")
			.ok()
			.unwrap();
		let gerald_eth = "0x6Be02d1d3665660d22FF9624b7BE0551ee1Ac91b".to_string();

		assert_ok!(create_dao(gerald));
		assert_ok!(approve_dao(
			H256::from_str("0x1a701f5720b125c5947bd7de0869d321785460533f4220c44b7cb1f10e91b0dd")
				.ok()
				.unwrap()
		));

		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);

		let pending_hash =
			H256::from_str("0xa97bb58cb53450f544cb331acc5d8844846c1b80456699a9939a5bf1bb1057d9")
				.ok()
				.unwrap();

		assert_ok!(DaoEthGovernance::propose(
			RuntimeOrigin::signed(gerald.clone()),
			0,
			Box::new(proposal.clone()),
			proposal_len,
			gerald_eth.clone().into()
		));

		assert_ok!(DaoEthGovernance::approve_propose(
			RawOrigin::None.into(),
			0,
			10,
			0,
			pending_hash.clone(),
			true
		));

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let proposal_hash = BlakeTwo256::hash_of(&proposal_bounded.unwrap());

		assert_ok!(DaoEthGovernance::vote(
			RuntimeOrigin::signed(gerald.clone()),
			0,
			proposal_hash,
			0,
			Vote { aye: true, balance: 10_u32.into() },
			gerald_eth.into()
		));

		let pending_vote_hash =
			H256::from_str("0xfd6540af73c6a68ab53b67cefd77c779eca8941f255cb03498ac2280424e4cc7")
				.ok()
				.unwrap();

		assert_ok!(DaoEthGovernance::approve_vote(
			RawOrigin::None.into(),
			0,
			pending_vote_hash,
			true
		));

		assert_eq!(DaoEthGovernance::pending_voting(0, pending_vote_hash), None);
	})
}

#[test]
fn close_works() {
	new_test_ext().execute_with(|| {
		let gerald = Public::from_str("5CNJv1vQjABY9W3BtsV2tzaLCjZepWXaYYzuDGWUUNVvMjcG")
			.ok()
			.unwrap();
		let gerald_eth = "0x6Be02d1d3665660d22FF9624b7BE0551ee1Ac91b".to_string();

		assert_ok!(create_dao(gerald));
		assert_ok!(approve_dao(
			H256::from_str("0x1a701f5720b125c5947bd7de0869d321785460533f4220c44b7cb1f10e91b0dd")
				.ok()
				.unwrap()
		));

		let proposal = make_proposal(42);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);

		let pending_hash =
			H256::from_str("0xa97bb58cb53450f544cb331acc5d8844846c1b80456699a9939a5bf1bb1057d9")
				.ok()
				.unwrap();

		assert_ok!(DaoEthGovernance::propose(
			RuntimeOrigin::signed(gerald.clone()),
			0,
			Box::new(proposal.clone()),
			proposal_len,
			gerald_eth.clone().into()
		));

		assert_ok!(DaoEthGovernance::approve_propose(
			RawOrigin::None.into(),
			0,
			10,
			0,
			pending_hash.clone(),
			true
		));

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let proposal_hash = BlakeTwo256::hash_of(&proposal_bounded.unwrap());

		assert_ok!(DaoEthGovernance::vote(
			RuntimeOrigin::signed(gerald.clone()),
			0,
			proposal_hash,
			0,
			Vote { aye: true, balance: 10_u32.into() },
			gerald_eth.into()
		));

		let pending_vote_hash =
			H256::from_str("0xfd6540af73c6a68ab53b67cefd77c779eca8941f255cb03498ac2280424e4cc7")
				.ok()
				.unwrap();

		assert_ok!(DaoEthGovernance::approve_vote(
			RawOrigin::None.into(),
			0,
			pending_vote_hash,
			true
		));

		let bounded_hash =
			H256::from_str("0x5351449fdb96b2e521ad3564da1f023266612394357a8c87be08f25a37cec7a8")
				.ok()
				.unwrap();

		assert_ok!(DaoEthGovernance::close(
			RuntimeOrigin::signed(gerald.clone()),
			0,
			bounded_hash,
			0,
			Weight::from_parts(u64::MAX, u64::MAX),
			1_024_u32
		));
	})
}
