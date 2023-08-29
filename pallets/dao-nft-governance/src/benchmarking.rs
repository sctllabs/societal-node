//! Benchmarking setup for pallet-dao-nft-governance

use super::*;

#[allow(unused)]
use crate::Pallet as DaoNftGovernance;
use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;

use serde_json::json;

const SEED: u32 = 0;

// Create the pre-requisite information needed to create a dao.
fn setup_dao_payload<T: Config>() -> Vec<u8> {
	let dao_json = json!({
		"name": "name",
		"purpose": "purpose",
		"metadata": "metadata",
		"policy": {
			"proposal_period": 300000,
		},
		"token": {
			"token_id": 0,
			"metadata": {
				"name": "token",
				"symbol": "symbol",
				"decimals": 2
			}
		}
	});

	serde_json::to_vec(&dao_json).ok().unwrap()
}

fn get_proposal_metadata() -> Vec<u8> {
	"Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor \
	incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud \
	exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure \
	dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. \
	Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt \
	mollit anim id est laborum. Lorem ipsum dolor sit amet, consectetur adipiscing elit, \
	sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim \
	veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo \
	consequat. Duis aute irure dolor in reprehenderit"
		.into()
}

fn create_dao<T: Config>() -> Result<T::AccountId, DispatchError> {
	let caller: T::AccountId = account("caller", 0, SEED);

	let data = setup_dao_payload::<T>();

	T::DaoProvider::create_dao(caller.clone(), vec![caller.clone()], vec![caller.clone()], data)?;

	Ok(caller)
}

fn funded_account<T: Config>(
	name: &'static str,
	index: u32,
) -> Result<T::AccountId, DispatchError> {
	let caller: T::AccountId = account(name, index, SEED);

	Ok(caller)
}

fn make_proposal<T: Config>(n: u32) -> T::Proposal {
	frame_system::Call::remark { remark: n.encode() }.into()
}

fn add_proposal<T: Config>(
	n: u32,
) -> Result<(T::Proposal, T::Hash, Bounded<T::Proposal>, T::Hash), &'static str> {
	let other = funded_account::<T>("proposer", n)?;
	let proposal = make_proposal::<T>(n);
	let bounded_proposal = <T as Config>::Preimages::bound(proposal.clone()).unwrap();
	DaoNftGovernance::<T>::propose_with_meta(
		RawOrigin::Signed(other).into(),
		0,
		Box::new(proposal.clone()),
		Some(get_proposal_metadata()),
	)?;
	Ok((
		proposal.clone(),
		T::Hashing::hash_of(&proposal).into(),
		bounded_proposal.clone(),
		T::Hashing::hash_of(&bounded_proposal).into(),
	))
}

benchmarks! {
	propose_with_meta {
		let p = T::MaxProposals::get();

		let caller = account("caller", 0, SEED);

		create_dao::<T>()?;

		for i in 1 .. (p - 1) {
			add_proposal::<T>(i)?;
		}

		let proposal = make_proposal::<T>(0);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let meta: Option<Vec<u8>> = Some(get_proposal_metadata());
	}: _(RawOrigin::Signed(caller), 0, Box::new(proposal), meta)
	verify { }

	// TODO: use max voters instead
	vote {
		let p = T::MaxProposals::get();

		let caller: T::AccountId = account("caller", 0, SEED);

		create_dao::<T>()?;

		let (proposal, proposal_hash, bounded_proposal, bounded_hash) = add_proposal::<T>(0)?;

	}: _(RawOrigin::Signed(caller), 0, bounded_hash, 0, Vote { aye: true, item: 0_u32.into() })
	verify { }

	close {
		let caller: T::AccountId = account("caller", 0, SEED);

		create_dao::<T>()?;

		let (proposal, proposal_hash, bounded_proposal, bounded_hash) = add_proposal::<T>(0)?;

		DaoNftGovernance::<T>::vote(
			RawOrigin::Signed(caller.clone()).into(),
			0,
			bounded_hash,
			0,
			Vote { aye: true, item: 0_u32.into() },
		)?;
	}: _(RawOrigin::Signed(caller.into()), 0, bounded_hash, 0, Weight::from_parts(u64::MAX, u64::MAX))
	verify {
		assert_eq!(Proposals::<T>::get(0).len(), usize::try_from(0_u32).unwrap());
		assert_eq!(ProposalOf::<T>::get(0, bounded_hash), None);
	}

	impl_benchmark_test_suite!(DaoNftGovernance, crate::mock::new_test_ext(), crate::mock::Test);
}
