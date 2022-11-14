use super::{Pallet as Membership, *};
use frame_benchmarking::{account, benchmarks_instance_pallet, whitelist};
use frame_support::{assert_ok, traits::EnsureOrigin};
use frame_system::RawOrigin;

const SEED: u32 = 0;

fn set_members<T: Config<I>, I: 'static>(members: Vec<T::AccountId>, prime: Option<usize>) {
	let approve_origin = T::ApproveOrigin::successful_origin(&(1, 1));

	assert_ok!(<Membership<T, I>>::reset_members(approve_origin.clone(), 0, members.clone()));
	if let Some(prime) = prime.map(|i| members[i].clone()) {
		assert_ok!(<Membership<T, I>>::set_prime(approve_origin.clone(), 0, prime));
	} else {
		assert_ok!(<Membership<T, I>>::clear_prime(approve_origin, 0));
	}
}

benchmarks_instance_pallet! {
	add_member {
		let m in 1 .. (T::MaxMembers::get() - 1);

		let members = (0..m).map(|i| account("member", i, SEED)).collect::<Vec<T::AccountId>>();
		set_members::<T, I>(members, None);
		let new_member = account::<T::AccountId>("add", m, SEED);
	}: {
		assert_ok!(<Membership<T, I>>::add_member(T::ApproveOrigin::successful_origin(&(1, 1)), 0, new_member.clone()));
	}
	verify {
		assert!(<Members<T, I>>::get(0).contains(&new_member));
		#[cfg(test)] crate::tests::clean();
	}

	// the case of no prime or the prime being removed is surely cheaper than the case of
	// reporting a new prime via `MembershipChanged`.
	remove_member {
		let m in 2 .. T::MaxMembers::get();

		let members = (0..m).map(|i| account("member", i, SEED)).collect::<Vec<T::AccountId>>();
		set_members::<T, I>(members.clone(), Some(members.len() - 1));

		let to_remove = members.first().cloned().unwrap();
	}: {
		assert_ok!(<Membership<T, I>>::remove_member(T::ApproveOrigin::successful_origin(&(1, 1)), 0, to_remove.clone()));
	} verify {
		assert!(!<Members<T, I>>::get(0).contains(&to_remove));
		// prime is rejigged
		assert!(<Prime<T, I>>::get(0).is_some() && T::MembershipChanged::get_prime(0).is_some());
		#[cfg(test)] crate::tests::clean();
	}

	// we remove a non-prime to make sure it needs to be set again.
	swap_member {
		let m in 2 .. T::MaxMembers::get();

		let members = (0..m).map(|i| account("member", i, SEED)).collect::<Vec<T::AccountId>>();
		set_members::<T, I>(members.clone(), Some(members.len() - 1));
		let add = account::<T::AccountId>("member", m, SEED);
		let remove = members.first().cloned().unwrap();
	}: {
		assert_ok!(<Membership<T, I>>::swap_member(
			T::ApproveOrigin::successful_origin(&(1, 1)),
			0,
			remove.clone(),
			add.clone(),
		));
	} verify {
		assert!(!<Members<T, I>>::get(0).contains(&remove));
		assert!(<Members<T, I>>::get(0).contains(&add));
		// prime is rejigged
		assert!(<Prime<T, I>>::get(0).is_some() && T::MembershipChanged::get_prime(0).is_some());
		#[cfg(test)] crate::tests::clean();
	}

	// er keep the prime common between incoming and outgoing to make sure it is rejigged.
	reset_member {
		let m in 1 .. T::MaxMembers::get();

		let members = (1..m+1).map(|i| account("member", i, SEED)).collect::<Vec<T::AccountId>>();
		set_members::<T, I>(members.clone(), Some(members.len() - 1));
		let mut new_members = (m..2*m).map(|i| account("member", i, SEED)).collect::<Vec<T::AccountId>>();
	}: {
		assert_ok!(<Membership<T, I>>::reset_members(T::ApproveOrigin::successful_origin(&(1, 1)), 0, new_members.clone()));
	} verify {
		new_members.sort();
		assert_eq!(<Members<T, I>>::get(0), new_members);
		// prime is rejigged
		assert!(<Prime<T, I>>::get(0).is_some() && T::MembershipChanged::get_prime(0).is_some());
		#[cfg(test)] crate::tests::clean();
	}

	change_key {
		let m in 1 .. T::MaxMembers::get();

		// worse case would be to change the prime
		let members = (0..m).map(|i| account("member", i, SEED)).collect::<Vec<T::AccountId>>();
		let prime = members.last().cloned().unwrap();
		set_members::<T, I>(members.clone(), Some(members.len() - 1));

		let add = account::<T::AccountId>("member", m, SEED);
		whitelist!(prime);
	}: {
		assert_ok!(<Membership<T, I>>::change_key(RawOrigin::Signed(prime.clone()).into(), 0, add.clone()));
	} verify {
		assert!(!<Members<T, I>>::get(0).contains(&prime));
		assert!(<Members<T, I>>::get(0).contains(&add));
		// prime is rejigged
		assert_eq!(<Prime<T, I>>::get(0).unwrap(), add);
		#[cfg(test)] crate::tests::clean();
	}

	set_prime {
		let m in 1 .. T::MaxMembers::get();
		let members = (0..m).map(|i| account("member", i, SEED)).collect::<Vec<T::AccountId>>();
		let prime = members.last().cloned().unwrap();
		set_members::<T, I>(members, None);
	}: {
		assert_ok!(<Membership<T, I>>::set_prime(T::ApproveOrigin::successful_origin(&(1, 1)), 0, prime));
	} verify {
		assert!(<Prime<T, I>>::get(0).is_some());
		assert!(<T::MembershipChanged>::get_prime(0).is_some());
		#[cfg(test)] crate::tests::clean();
	}

	clear_prime {
		let m in 1 .. T::MaxMembers::get();
		let members = (0..m).map(|i| account("member", i, SEED)).collect::<Vec<T::AccountId>>();
		let prime = members.last().cloned().unwrap();
		set_members::<T, I>(members, None);
	}: {
		assert_ok!(<Membership<T, I>>::clear_prime(T::ApproveOrigin::successful_origin(&(1, 1)), 0));
	} verify {
		assert!(<Prime<T, I>>::get(0).is_none());
		assert!(<T::MembershipChanged>::get_prime(0).is_none());
		#[cfg(test)] crate::tests::clean();
	}

	impl_benchmark_test_suite!(Membership, crate::tests::new_bench_ext(), crate::tests::Test);
}
