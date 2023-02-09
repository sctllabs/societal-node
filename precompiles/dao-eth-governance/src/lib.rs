#![cfg_attr(not(feature = "std"), no_std)]

extern crate core;

use fp_evm::{Log, PrecompileHandle};
use frame_support::{
	dispatch::{Dispatchable, GetDispatchInfo, Pays, PostDispatchInfo},
	weights::Weight,
};
use pallet_dao_eth_governance::vote::Vote;
use pallet_evm::AddressMapping;
use parity_scale_codec::Decode;
use precompile_utils::{helpers::hash, prelude::*};
use sp_core::{ConstU32, H160, H256};
use sp_std::{boxed::Box, marker::PhantomData, vec::Vec};

/// Dao ID. Just a `u32`.
pub type DaoId = u32;

type BalanceOf<Runtime> = <Runtime as pallet_dao_eth_governance::Config>::Balance;

type GetProposalLimit = ConstU32<100>;
type GetAccountIdLimit = ConstU32<42>;
type GetProposalMetaLimit = ConstU32<100>;

/// Solidity selector of the Executed log.
pub const SELECTOR_LOG_EXECUTED: [u8; 32] = keccak256!("Executed(bytes32)");

/// Solidity selector of the Proposed log.
pub const SELECTOR_LOG_PROPOSED: [u8; 32] =
	keccak256!("Proposed(address,uint32,uint32,bytes32,uint32)");

/// Solidity selector of the Voted log.
pub const SELECTOR_LOG_VOTED: [u8; 32] = keccak256!("Voted(address,uint32,bytes32,bool,uint128)");

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
	threshold: u32,
) -> Log {
	log5(
		address.into(),
		SELECTOR_LOG_PROPOSED,
		who.into(),
		H256::from_slice(&EvmDataWriter::new().write(dao_id).build()),
		H256::from_slice(&EvmDataWriter::new().write(index).build()),
		hash,
		EvmDataWriter::new().write(threshold).build(),
	)
}

pub fn log_voted(
	address: impl Into<H160>,
	who: impl Into<H160>,
	dao_id: DaoId,
	hash: H256,
	aye: bool,
	balance: u128,
) -> Log {
	log5(
		address.into(),
		SELECTOR_LOG_VOTED,
		who.into(),
		H256::from_slice(&EvmDataWriter::new().write(dao_id).build()),
		hash,
		H256::from_slice(&EvmDataWriter::new().write(aye).build()),
		EvmDataWriter::new().write(balance).build(),
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
pub struct DaoEthGovernancePrecompile<Runtime>(PhantomData<Runtime>);

#[precompile_utils::precompile]
impl<Runtime> DaoEthGovernancePrecompile<Runtime>
where
	Runtime: pallet_dao_eth_governance::Config + pallet_evm::Config,
	Runtime::RuntimeCall: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo + Decode,
	Runtime::RuntimeCall: From<pallet_dao_eth_governance::Call<Runtime>>,
	<Runtime as pallet_dao_eth_governance::Config>::Proposal: From<Runtime::RuntimeCall>,
	<Runtime::RuntimeCall as Dispatchable>::RuntimeOrigin: From<Option<Runtime::AccountId>>,
	BalanceOf<Runtime>: TryFrom<u128> + TryInto<u128> + Into<u128> + EvmData,
	H256: From<<Runtime as frame_system::Config>::Hash>
		+ Into<<Runtime as frame_system::Config>::Hash>,
{
	#[precompile::public("propose(uint32,bytes,bytes)")]
	fn propose(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		proposal: BoundedBytes<GetProposalLimit>,
		account_id: BoundedBytes<GetAccountIdLimit>,
	) -> EvmResult<u32> {
		Self::propose_with_meta(handle, dao_id, proposal, account_id, BoundedBytes::from(""))
	}

	#[precompile::public("propose(uint32,bytes,bytes,bytes)")]
	fn propose_with_meta(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		proposal: BoundedBytes<GetProposalLimit>,
		account_id: BoundedBytes<GetAccountIdLimit>,
		meta: BoundedBytes<GetProposalMetaLimit>,
	) -> EvmResult<u32> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		let proposal: Vec<_> = proposal.into();
		let proposal_length: u32 = proposal.len().try_into().map_err(|_| {
			RevertReason::value_is_too_large("uint32")
				.in_field("length")
				.in_field("proposal")
		})?;

		let proposal_index = pallet_dao_eth_governance::Pallet::<Runtime>::proposal_count(dao_id);
		let _proposal_hash: H256 = hash::<Runtime>(&proposal);
		let proposal = Runtime::RuntimeCall::decode(&mut &*proposal)
			.map_err(|_| RevertReason::custom("Failed to decode proposal").in_field("proposal"))?
			.into();
		let proposal = Box::new(proposal);

		{
			let origin = Runtime::AddressMapping::into_account_id(handle.context().caller);
			RuntimeHelper::<Runtime>::try_dispatch(
				handle,
				Some(origin).into(),
				pallet_dao_eth_governance::Call::<Runtime>::propose_with_meta {
					dao_id,
					proposal,
					length_bound: proposal_length,
					account_id: account_id.into(),
					meta: Some(meta.into()),
				},
			)?;
		}

		Ok(proposal_index)
	}

	#[precompile::public("vote(uint32,bytes32,uint32,bool,uint128,bytes)")]
	fn vote(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		proposal_hash: H256,
		proposal_index: u32,
		aye: bool,
		balance: u128,
		account_id: BoundedBytes<GetAccountIdLimit>,
	) -> EvmResult {
		let origin = Runtime::AddressMapping::into_account_id(handle.context().caller);

		let vote = Vote { aye, balance: Self::u128_to_amount(balance).in_field("balance")? };
		RuntimeHelper::<Runtime>::try_dispatch(
			handle,
			Some(origin).into(),
			pallet_dao_eth_governance::Call::<Runtime>::vote {
				dao_id,
				proposal: proposal_hash.into(),
				index: proposal_index,
				vote,
				account_id: account_id.into(),
			},
		)?;

		let log = log_voted(
			handle.context().address,
			handle.context().caller,
			dao_id,
			proposal_hash,
			aye,
			balance,
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
			pallet_dao_eth_governance::Call::<Runtime>::close {
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

		let proposals = pallet_dao_eth_governance::Pallet::<Runtime>::proposals(dao_id);
		let proposals: Vec<_> = proposals.into_iter().map(|hash| hash.into()).collect();

		Ok(proposals)
	}

	fn u128_to_amount(value: u128) -> MayRevert<BalanceOf<Runtime>> {
		value
			.try_into()
			.map_err(|_| RevertReason::value_is_too_large("balance type").into())
	}
}
