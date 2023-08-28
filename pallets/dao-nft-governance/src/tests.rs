use crate::{mock::*, Vote};
use frame_support::{
	assert_ok,
	traits::{Bounded, StorePreimage},
	weights::Weight,
};
use sp_core::{sr25519::Public, H256};
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

fn create_dao(_who: Public) -> DispatchResult {
	Ok(())
}

fn approve_dao(_hash: H256) -> DispatchResult {
	Ok(())
}

#[test]
fn propose_works() {
	new_test_ext().execute_with(|| {
		let gerald = Public::from_str("5CNJv1vQjABY9W3BtsV2tzaLCjZepWXaYYzuDGWUUNVvMjcG")
			.ok()
			.unwrap();

		assert_ok!(create_dao(gerald));

		let proposal = make_proposal(42);

		assert_ok!(DaoNftGovernance::propose(
			RuntimeOrigin::signed(gerald.clone()),
			0,
			Box::new(proposal.clone()),
		));
	})
}

#[test]
fn vote_works() {
	new_test_ext().execute_with(|| {
		let gerald = Public::from_str("5CNJv1vQjABY9W3BtsV2tzaLCjZepWXaYYzuDGWUUNVvMjcG")
			.ok()
			.unwrap();

		assert_ok!(create_dao(gerald));
		assert_ok!(approve_dao(
			H256::from_str("0x1a701f5720b125c5947bd7de0869d321785460533f4220c44b7cb1f10e91b0dd")
				.ok()
				.unwrap()
		));

		let proposal = make_proposal(42);

		assert_ok!(DaoNftGovernance::propose(
			RuntimeOrigin::signed(gerald.clone()),
			0,
			Box::new(proposal.clone()),
		));

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let proposal_hash = BlakeTwo256::hash_of(&proposal_bounded.unwrap());

		assert_ok!(DaoNftGovernance::vote(
			RuntimeOrigin::signed(gerald.clone()),
			0,
			proposal_hash,
			0,
			Vote { aye: true, item: 0 },
		));
	})
}

#[test]
fn close_works() {
	new_test_ext().execute_with(|| {
		let gerald = Public::from_str("5CNJv1vQjABY9W3BtsV2tzaLCjZepWXaYYzuDGWUUNVvMjcG")
			.ok()
			.unwrap();

		assert_ok!(create_dao(gerald));
		assert_ok!(approve_dao(
			H256::from_str("0x1a701f5720b125c5947bd7de0869d321785460533f4220c44b7cb1f10e91b0dd")
				.ok()
				.unwrap()
		));

		let proposal = make_proposal(42);

		assert_ok!(DaoNftGovernance::propose(
			RuntimeOrigin::signed(gerald.clone()),
			0,
			Box::new(proposal.clone()),
		));

		let proposal_bounded: Option<Bounded<RuntimeCall>> = match Preimage::bound(proposal.clone())
		{
			Ok(bounded) => Some(bounded.transmute()),
			Err(_) => None,
		};

		let proposal_hash = BlakeTwo256::hash_of(&proposal_bounded.unwrap());

		assert_ok!(DaoNftGovernance::vote(
			RuntimeOrigin::signed(gerald.clone()),
			0,
			proposal_hash,
			0,
			Vote { aye: true, item: 0 },
		));

		let bounded_hash =
			H256::from_str("0x5351449fdb96b2e521ad3564da1f023266612394357a8c87be08f25a37cec7a8")
				.ok()
				.unwrap();

		assert_ok!(DaoNftGovernance::close(
			RuntimeOrigin::signed(gerald.clone()),
			0,
			bounded_hash,
			0,
			Weight::from_parts(u64::MAX, u64::MAX),
		));
	})
}
