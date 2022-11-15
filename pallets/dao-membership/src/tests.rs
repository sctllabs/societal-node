use crate::{mock::*, Error};
use frame_support::{assert_noop, assert_ok, bounded_vec};
use sp_runtime::traits::BadOrigin;
use std::collections::HashMap;

use dao_primitives::InitializeDaoMembers;

pub(crate) fn init_members() {
	let mut members: HashMap<u32, Vec<u64>> = HashMap::new();
	members.insert(0, vec![10, 20, 30]);

	Members::set(members);

	Membership::initialize_members(0, vec![10, 20, 30]).ok();
}

#[cfg(feature = "runtime-benchmarks")]
pub(crate) fn new_bench_ext() -> sp_io::TestExternalities {
	frame_system::GenesisConfig::default().build_storage::<Test>().unwrap().into()
}

#[cfg(feature = "runtime-benchmarks")]
pub(crate) fn clean() {
	Members::set(HashMap::new());
}

#[test]
fn query_membership_works() {
	new_test_ext().execute_with(|| {
		init_members();

		assert_eq!(Membership::members(0), vec![10, 20, 30]);
		assert_eq!(MEMBERS.with(|m| m.borrow().clone()).get(&0).unwrap().clone(), vec![10, 20, 30]);
	});
}

#[test]
fn add_member_works() {
	new_test_ext().execute_with(|| {
		init_members();

		assert_noop!(Membership::add_member(Origin::signed(5), 0, 15), BadOrigin);
		assert_noop!(
			Membership::add_member(Origin::signed(1), 0, 10),
			Error::<Test, _>::AlreadyMember
		);
		assert_ok!(Membership::add_member(Origin::signed(1), 0, 15));
		assert_eq!(Membership::members(0), vec![10, 15, 20, 30]);
		assert_eq!(
			MEMBERS.with(|m| m.borrow().clone()).get(&0).unwrap().clone(),
			Membership::members(0).to_vec()
		);
	});
}

#[test]
fn remove_member_works() {
	new_test_ext().execute_with(|| {
		init_members();

		assert_noop!(Membership::remove_member(Origin::signed(5), 0, 20), BadOrigin);
		assert_noop!(
			Membership::remove_member(Origin::signed(1), 0, 15),
			Error::<Test, _>::NotMember
		);
		assert_ok!(Membership::remove_member(Origin::signed(1), 0, 20));
		assert_eq!(Membership::members(0), vec![10, 30]);
		assert_eq!(
			MEMBERS.with(|m| m.borrow().clone()).get(&0).unwrap().clone(),
			Membership::members(0).to_vec()
		);
	});
}

#[test]
fn swap_member_works() {
	new_test_ext().execute_with(|| {
		init_members();

		assert_noop!(Membership::swap_member(Origin::signed(5), 0, 10, 25), BadOrigin);
		assert_noop!(
			Membership::swap_member(Origin::signed(1), 0, 15, 25),
			Error::<Test, _>::NotMember
		);
		assert_noop!(
			Membership::swap_member(Origin::signed(1), 0, 10, 30),
			Error::<Test, _>::AlreadyMember
		);

		assert_ok!(Membership::swap_member(Origin::signed(1), 0, 20, 20));
		assert_eq!(Membership::members(0), vec![10, 20, 30]);

		assert_ok!(Membership::swap_member(Origin::signed(1), 0, 10, 25));
		assert_eq!(Membership::members(0), vec![20, 25, 30]);
		assert_eq!(
			MEMBERS.with(|m| m.borrow().clone()).get(&0).unwrap().clone(),
			Membership::members(0).to_vec()
		);
	});
}

#[test]
fn swap_member_works_that_does_not_change_order() {
	new_test_ext().execute_with(|| {
		init_members();

		assert_ok!(Membership::swap_member(Origin::signed(1), 0, 10, 5));
		assert_eq!(Membership::members(0), vec![5, 20, 30]);
		assert_eq!(
			MEMBERS.with(|m| m.borrow().clone()).get(&0).unwrap().clone(),
			Membership::members(0).to_vec()
		);
	});
}

#[test]
fn change_key_works() {
	new_test_ext().execute_with(|| {
		init_members();

		Membership::add_member(Origin::signed(1), 0, 1).ok();
		assert_noop!(
			Membership::change_key(Origin::signed(1), 0, 20),
			Error::<Test, _>::AlreadyMember
		);
		assert_ok!(Membership::change_key(Origin::signed(1), 0, 40));
		assert_eq!(Membership::members(0), vec![10, 20, 30, 40]);
		assert_eq!(
			MEMBERS.with(|m| m.borrow().clone()).get(&0).unwrap().clone(),
			Membership::members(0).to_vec()
		);
	});
}

#[test]
fn change_key_works_that_does_not_change_order() {
	new_test_ext().execute_with(|| {
		init_members();

		Membership::add_member(Origin::signed(1), 0, 1).ok();

		assert_ok!(Membership::change_key(Origin::signed(1), 0, 5));
		assert_eq!(
			MEMBERS.with(|m| m.borrow().clone()).get(&0).unwrap().clone(),
			Membership::members(0).to_vec()
		);
	});
}

#[test]
fn reset_members_works() {
	new_test_ext().execute_with(|| {
		init_members();

		assert_noop!(
			Membership::reset_members(Origin::signed(10), 0, bounded_vec![20, 40, 30]),
			BadOrigin
		);

		assert_ok!(Membership::reset_members(Origin::signed(1), 0, vec![20, 40, 30]));
		assert_eq!(Membership::members(0), vec![20, 30, 40]);
		assert_eq!(
			MEMBERS.with(|m| m.borrow().clone()).get(&0).unwrap().clone(),
			Membership::members(0).to_vec()
		);

		assert_ok!(Membership::reset_members(Origin::signed(1), 0, vec![10, 40, 30]));
		assert_eq!(Membership::members(0), vec![10, 30, 40]);
		assert_eq!(
			MEMBERS.with(|m| m.borrow().clone()).get(&0).unwrap().clone(),
			Membership::members(0).to_vec()
		);
	});
}
