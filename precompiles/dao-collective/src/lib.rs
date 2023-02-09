#![cfg_attr(not(feature = "std"), no_std)]

extern crate core;

use fp_evm::{Log, PrecompileHandle};
use frame_support::{
	dispatch::{Dispatchable, GetDispatchInfo, Pays, PostDispatchInfo},
	weights::Weight,
};
use pallet_evm::AddressMapping;
use parity_scale_codec::Decode;
use precompile_utils::{helpers::hash, prelude::*};
use sp_core::{ConstU32, H160, H256};
use sp_std::{boxed::Box, marker::PhantomData, vec::Vec};

/// Dao ID. Just a `u32`.
pub type DaoId = u32;

type GetProposalLimit = ConstU32<100>;
type GetProposalMetaLimit = ConstU32<100>;

/// Solidity selector of the Executed log.
pub const SELECTOR_LOG_EXECUTED: [u8; 32] = keccak256!("Executed(bytes32)");

/// Solidity selector of the Proposed log.
pub const SELECTOR_LOG_PROPOSED: [u8; 32] =
	keccak256!("Proposed(address,uint32,uint32,bytes32,uint32)");

/// Solidity selector of the Voted log.
pub const SELECTOR_LOG_VOTED: [u8; 32] = keccak256!("Voted(address,uint32,bytes32,bool)");

/// Solidity selector of the Closed log.
pub const SELECTOR_LOG_CLOSED: [u8; 32] = keccak256!("Closed(uint32,bytes32)");

pub fn log_executed(address: impl Into<H160>, dao_id: DaoId, hash: H256) -> Log {
	log3(
		address.into(),
		SELECTOR_LOG_EXECUTED,
		H256::from_slice(&EvmDataWriter::new().write(dao_id).build()),
		hash,
		Vec::new(),
	)
}

pub fn log_proposed(
	address: impl Into<H160>,
	who: impl Into<H160>,
	dao_id: DaoId,
	index: u32,
	hash: H256,
) -> Log {
	log4(
		address.into(),
		SELECTOR_LOG_PROPOSED,
		who.into(),
		H256::from_slice(&EvmDataWriter::new().write(dao_id).build()),
		H256::from_slice(&EvmDataWriter::new().write(index).build()),
		EvmDataWriter::new().write(hash).build(),
	)
}

pub fn log_voted(
	address: impl Into<H160>,
	who: impl Into<H160>,
	dao_id: DaoId,
	hash: H256,
	voted: bool,
) -> Log {
	log4(
		address.into(),
		SELECTOR_LOG_VOTED,
		who.into(),
		H256::from_slice(&EvmDataWriter::new().write(dao_id).build()),
		hash,
		EvmDataWriter::new().write(voted).build(),
	)
}

pub fn log_closed(address: impl Into<H160>, dao_id: DaoId, hash: H256) -> Log {
	log3(
		address.into(),
		SELECTOR_LOG_CLOSED,
		H256::from_slice(&EvmDataWriter::new().write(dao_id).build()),
		hash,
		Vec::new(),
	)
}

/// A precompile to wrap the functionality from pallet-dao-collective.
pub struct DaoCollectivePrecompile<Runtime, Instance: 'static>(PhantomData<(Runtime, Instance)>);

#[precompile_utils::precompile]
impl<Runtime, Instance> DaoCollectivePrecompile<Runtime, Instance>
where
	Instance: 'static,
	Runtime: pallet_dao_collective::Config<Instance> + pallet_evm::Config,
	Runtime::RuntimeCall: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo + Decode,
	Runtime::RuntimeCall: From<pallet_dao_collective::Call<Runtime, Instance>>,
	<Runtime as pallet_dao_collective::Config<Instance>>::Proposal: From<Runtime::RuntimeCall>,
	<Runtime::RuntimeCall as Dispatchable>::RuntimeOrigin: From<Option<Runtime::AccountId>>,
	H256: From<<Runtime as frame_system::Config>::Hash>
		+ Into<<Runtime as frame_system::Config>::Hash>,
{
	#[precompile::public("execute(uint32,bytes)")]
	fn execute(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		proposal: BoundedBytes<GetProposalLimit>,
	) -> EvmResult {
		let proposal: Vec<_> = proposal.into();
		let proposal_hash: H256 = hash::<Runtime>(&proposal);
		let proposal_length: u32 = proposal.len().try_into().map_err(|_| {
			RevertReason::value_is_too_large("uint32")
				.in_field("length")
				.in_field("proposal")
		})?;

		let proposal = Runtime::RuntimeCall::decode(&mut &*proposal)
			.map_err(|_| RevertReason::custom("Failed to decode proposal").in_field("proposal"))?
			.into();
		let proposal = Box::new(proposal);

		let origin = Runtime::AddressMapping::into_account_id(handle.context().caller);
		RuntimeHelper::<Runtime>::try_dispatch(
			handle,
			Some(origin).into(),
			pallet_dao_collective::Call::<Runtime, Instance>::execute {
				dao_id,
				proposal,
				length_bound: proposal_length,
			},
		)?;

		let log = log_executed(handle.context().address, dao_id, proposal_hash);

		handle.record_log_costs(&[&log])?;
		log.record(handle)?;

		Ok(())
	}

	#[precompile::public("propose(uint32,bytes)")]
	fn propose(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		proposal: BoundedBytes<GetProposalLimit>,
	) -> EvmResult<u32> {
		Self::propose_with_meta(handle, dao_id, proposal, BoundedBytes::from(""))
	}

	#[precompile::public("propose(uint32,bytes,bytes)")]
	fn propose_with_meta(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		proposal: BoundedBytes<GetProposalLimit>,
		meta: BoundedBytes<GetProposalMetaLimit>,
	) -> EvmResult<u32> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		let proposal: Vec<_> = proposal.into();
		let proposal_length: u32 = proposal.len().try_into().map_err(|_| {
			RevertReason::value_is_too_large("uint32")
				.in_field("length")
				.in_field("proposal")
		})?;

		let proposal_index =
			pallet_dao_collective::Pallet::<Runtime, Instance>::proposal_count(dao_id);
		let proposal_hash: H256 = hash::<Runtime>(&proposal);
		let proposal = Runtime::RuntimeCall::decode(&mut &*proposal)
			.map_err(|_| RevertReason::custom("Failed to decode proposal").in_field("proposal"))?
			.into();
		let proposal = Box::new(proposal);

		{
			let origin = Runtime::AddressMapping::into_account_id(handle.context().caller);
			RuntimeHelper::<Runtime>::try_dispatch(
				handle,
				Some(origin).into(),
				pallet_dao_collective::Call::<Runtime, Instance>::propose_with_meta {
					dao_id,
					proposal,
					length_bound: proposal_length,
					meta: Some(meta.into()),
				},
			)?;
		}

		let log = log_proposed(
			handle.context().address,
			handle.context().caller,
			dao_id,
			proposal_index,
			proposal_hash,
		);

		handle.record_log_costs(&[&log])?;
		log.record(handle)?;

		Ok(proposal_index)
	}

	#[precompile::public("vote(uint32,bytes32,uint32,bool)")]
	fn vote(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		proposal_hash: H256,
		proposal_index: u32,
		approve: bool,
	) -> EvmResult {
		let origin = Runtime::AddressMapping::into_account_id(handle.context().caller);
		RuntimeHelper::<Runtime>::try_dispatch(
			handle,
			Some(origin).into(),
			pallet_dao_collective::Call::<Runtime, Instance>::vote {
				dao_id,
				proposal: proposal_hash.into(),
				index: proposal_index,
				approve,
			},
		)?;

		// TODO: Since we cannot access ayes/nays of a proposal we cannot
		// include it in the EVM events to mirror Substrate events.

		let log = log_voted(
			handle.context().address,
			handle.context().caller,
			dao_id,
			proposal_hash,
			approve,
		);
		handle.record_log_costs(&[&log])?;
		log.record(handle)?;

		Ok(())
	}

	#[precompile::public("close(uint32,bytes32,uint32,uint64,uint32)")]
	fn close(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		proposal_hash: H256,
		proposal_index: u32,
		proposal_weight_bound: u64,
		length_bound: u32,
	) -> EvmResult<bool> {
		let origin = Runtime::AddressMapping::into_account_id(handle.context().caller);
		let post_dispatch_info = RuntimeHelper::<Runtime>::try_dispatch(
			handle,
			Some(origin).into(),
			pallet_dao_collective::Call::<Runtime, Instance>::close {
				dao_id,
				proposal_hash: proposal_hash.into(),
				index: proposal_index,
				proposal_weight_bound: Weight::from_ref_time(proposal_weight_bound),
				length_bound,
			},
		)?;

		// We can know if the proposal was executed or not based on the `pays_fee` in
		// `PostDispatchInfo`.
		let (executed, log) = match post_dispatch_info.pays_fee {
			Pays::Yes => (true, log_executed(handle.context().address, dao_id, proposal_hash)),
			Pays::No => (false, log_closed(handle.context().address, dao_id, proposal_hash)),
		};
		handle.record_log_costs(&[&log])?;
		log.record(handle)?;

		Ok(executed)
	}

	#[precompile::public("proposalHash(bytes)")]
	#[precompile::view]
	fn proposal_hash(
		_handle: &mut impl PrecompileHandle,
		proposal: BoundedBytes<GetProposalLimit>,
	) -> EvmResult<H256> {
		let proposal: Vec<_> = proposal.into();
		let hash = hash::<Runtime>(&proposal);

		Ok(hash)
	}

	#[precompile::public("proposals(uint32)")]
	#[precompile::view]
	fn proposals(handle: &mut impl PrecompileHandle, dao_id: DaoId) -> EvmResult<Vec<H256>> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		let proposals = pallet_dao_collective::Pallet::<Runtime, Instance>::proposals(dao_id);
		let proposals: Vec<_> = proposals.into_iter().map(|hash| hash.into()).collect();

		Ok(proposals)
	}

	// #[precompile::public("members()")]
	// #[precompile::view]
	// fn members(handle: &mut impl PrecompileHandle, dao_id: DaoId) -> EvmResult<Vec<Address>> {
	// 	handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
	//
	// 	let members = pallet_dao_collective::Pallet::<Runtime, Instance>::members(dao_id);
	// 	let members: Vec<_> = members.into_iter().map(|id| Address(id.into())).collect();
	//
	// 	Ok(members)
	// }

	#[precompile::public("isMember(uint32,address)")]
	#[precompile::view]
	fn is_member(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		account: Address,
	) -> EvmResult<bool> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		let account = Runtime::AddressMapping::into_account_id(account.into());

		let is_member =
			pallet_dao_collective::Pallet::<Runtime, Instance>::is_member(dao_id, &account);

		Ok(is_member)
	}
}
