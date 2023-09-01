//! Benchmarking setup for pallet-dao-eth-governance

use super::*;

#[allow(unused)]
use crate::Pallet as DaoEthGovernance;
use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;

use serde_json::json;

use codec::alloc::string::ToString;
use dao_primitives::{Dao, DaoConfig, DaoStatus};

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
		"token_address": "0x439ACbC2FAE8E4b9115e702AeBeAa9977621017C"
	});

	serde_json::to_vec(&dao_json).ok().unwrap()
}

fn create_dao<T: Config>() -> Result<T::AccountId, DispatchError> {
	let caller: T::AccountId = account("caller", 0, SEED);

	let data = setup_dao_payload::<T>();

	T::DaoProvider::create_dao(caller.clone(), vec![caller.clone()], vec![caller.clone()], data)?;

	let dao_account_id = T::DaoProvider::dao_account_id(0);
	let config = DaoConfig { name: "name", purpose: "purpose", metadata: "metadata" };
	let dao = Dao {
		founder: caller.clone(),
		config,
		account_id: dao_account_id,
		token: DaoToken::<u128, &str>::EthTokenAddress(
			"0x439ACbC2FAE8E4b9115e702AeBeAa9977621017C".into(),
		),
		status: DaoStatus::Pending,
	};

	T::DaoProvider::approve_dao(T::Hashing::hash_of(&dao).into(), true)?;

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
	let other_eth = "0x6Be02d1d3665660d22FF9624b7BE0551ee1Ac91b".to_string();
	let proposal = make_proposal::<T>(n);
	let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
	let bounded_proposal = <T as Config>::Preimages::bound(proposal.clone()).unwrap();
	DaoEthGovernance::<T>::propose_with_meta(
		RawOrigin::Signed(other).into(),
		0,
		Box::new(proposal.clone()),
		proposal_len,
		other_eth.into(),
		Some(vec![97_u8; T::ProposalMetadataLimit::get() as usize]),
	)?;
	Ok((
		proposal.clone(),
		T::Hashing::hash_of(&proposal).into(),
		bounded_proposal.clone(),
		T::Hashing::hash_of(&bounded_proposal).into(),
	))
}

fn add_approve_proposal<T: Config>(
	n: u32,
	threshold: u128,
) -> Result<(T::Proposal, T::Hash, Bounded<T::Proposal>, T::Hash), &'static str> {
	let (proposal, proposal_hash, bounded_proposal, bounded_hash) = add_proposal::<T>(n)?;

	DaoEthGovernance::<T>::approve_propose(
		RawOrigin::None.into(),
		0,
		threshold,
		0,
		proposal_hash,
		true,
	)?;

	Ok((proposal, proposal_hash, bounded_proposal, bounded_hash))
}

benchmarks! {
	propose_with_meta {
		let p = T::MaxProposals::get();

		let caller = account("caller", 0, SEED);
		let caller_eth = "0x6Be02d1d3665660d22FF9624b7BE0551ee1Ac91b".to_string();

		create_dao::<T>()?;

		for i in 0 .. (p - 1) {
			add_proposal::<T>(i)?;
		}

		let proposal = make_proposal::<T>(0);
		let proposal_len: u32 = proposal.using_encoded(|p| p.len() as u32);
		let meta = Some(vec![97_u8; T::ProposalMetadataLimit::get() as usize]);
	}: _(RawOrigin::Signed(caller), 0, Box::new(proposal), proposal_len, caller_eth.into(), meta)
	verify {
		assert_eq!(PendingProposals::<T>::get(0).len(), usize::try_from(T::MaxProposals::get()).unwrap());
	}

	approve_propose {
		let p = T::MaxProposals::get();

		create_dao::<T>()?;

		for i in 0 .. (p - 1) {
			add_approve_proposal::<T>(i, 10_u128)?;
		}

		let (proposal, proposal_hash, bounded_proposal, bounded_hash) = add_proposal::<T>(p)?;
	}: _(RawOrigin::None, 0, 10_u128, 0, proposal_hash, true)
	verify {
		assert_eq!(PendingProposals::<T>::get(0).len(), usize::try_from(0_u32).unwrap());
		assert_eq!(Proposals::<T>::get(0).len(), usize::try_from(T::MaxProposals::get()).unwrap());
		assert_eq!(ProposalOf::<T>::get(0, bounded_hash).unwrap(), bounded_proposal);
	}

	// TODO: use max voters instead
	vote {
		let p = T::MaxProposals::get();

		let caller: T::AccountId = account("caller", 0, SEED);
		let caller_eth = "0x6Be02d1d3665660d22FF9624b7BE0551ee1Ac91b".to_string();

		create_dao::<T>()?;

		let (proposal, proposal_hash, bounded_proposal, bounded_hash) = add_approve_proposal::<T>(0, 10_u128)?;

		let pending_vote: PendingVote<T::AccountId, T::Hash, BalanceOf<T>, T::BlockNumber> = PendingVote {
			who: caller.clone(),
			proposal_hash: bounded_hash,
			proposal_index: 0,
			aye: true,
			balance: 10_u32.into(),
			block_number: 1_u32.into(),
		};
		let pending_vote_hash = T::Hashing::hash_of(&pending_vote);
	}: _(RawOrigin::Signed(caller), 0, bounded_hash, 0, Vote { aye: true, balance: 10_u32.into() }, caller_eth.into())
	verify {
		assert_eq!(PendingVotes::<T>::get((0, 0)).len(), usize::try_from(1_u32).unwrap());
		assert_eq!(PendingVoting::<T>::get(0, pending_vote_hash).unwrap(), pending_vote);
	}

	approve_vote {
		let caller: T::AccountId = account("caller", 0, SEED);
		let caller_eth = "0x6Be02d1d3665660d22FF9624b7BE0551ee1Ac91b".to_string();

		create_dao::<T>()?;

		let (proposal, proposal_hash, bounded_proposal, bounded_hash) = add_approve_proposal::<T>(0, 10_u128)?;

		DaoEthGovernance::<T>::vote(
			RawOrigin::Signed(caller.clone()).into(),
			0,
			bounded_hash,
			0,
			Vote { aye: true, balance: 10_u32.into() },
			caller_eth.into()
		)?;

		let pending_vote: PendingVote<T::AccountId, T::Hash, BalanceOf<T>, T::BlockNumber> = PendingVote {
			who: caller.clone(),
			proposal_hash: bounded_hash,
			proposal_index: 0,
			aye: true,
			balance: 10_u32.into(),
			block_number: 0_u32.into(),
		};
		let pending_vote_hash = T::Hashing::hash_of(&pending_vote);
	}: _(RawOrigin::None, 0, pending_vote_hash, true)
	verify {
		assert_eq!(PendingVotes::<T>::get((0, 0)).len(), usize::try_from(0_u32).unwrap());
	}

	close {
		let caller: T::AccountId = account("caller", 0, SEED);
		let caller_eth = "0x6Be02d1d3665660d22FF9624b7BE0551ee1Ac91b".to_string();

		create_dao::<T>()?;

		let (proposal, proposal_hash, bounded_proposal, bounded_hash) = add_approve_proposal::<T>(0, 10_u128)?;

		DaoEthGovernance::<T>::vote(
			RawOrigin::Signed(caller.clone()).into(),
			0,
			bounded_hash,
			0,
			Vote { aye: true, balance: 10_u32.into() },
			caller_eth.into()
		)?;

		let pending_vote: PendingVote<T::AccountId, T::Hash, BalanceOf<T>, T::BlockNumber> = PendingVote {
			who: caller.clone(),
			proposal_hash: bounded_hash,
			proposal_index: 0,
			aye: true,
			balance: 10_u32.into(),
			block_number: 0_u32.into(),
		};
		let pending_vote_hash = T::Hashing::hash_of(&pending_vote);

		DaoEthGovernance::<T>::approve_vote(
			RawOrigin::None.into(),
			0,
			pending_vote_hash,
			true
		)?;
	}: _(RawOrigin::Signed(caller.into()), 0, bounded_hash, 0, Weight::from_parts(u64::MAX, u64::MAX), 1_024_u32)
	verify {
		assert_eq!(Proposals::<T>::get(0).len(), usize::try_from(0_u32).unwrap());
		assert_eq!(ProposalOf::<T>::get(0, bounded_hash), None);
	}

	impl_benchmark_test_suite!(DaoEthGovernance, crate::mock::new_test_ext(), crate::mock::Test);
}
