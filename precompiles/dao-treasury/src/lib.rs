#![cfg_attr(not(feature = "std"), no_std)]

extern crate core;

use fp_evm::{Log, PrecompileHandle};
use frame_support::dispatch::{Dispatchable, GetDispatchInfo, PostDispatchInfo};
use pallet_evm::AddressMapping;
use precompile_utils::prelude::*;
use sp_core::{H160, H256};
use sp_runtime::traits::StaticLookup;
use sp_std::{marker::PhantomData, vec::Vec};

/// Dao ID. Just a `u32`.
pub type DaoId = u32;

/// An index of a proposal. Just a `u32`.
pub type ProposalIndex = u32;

/// Solidity selector of the Proposed log.
pub const SELECTOR_LOG_PROPOSED: [u8; 32] = keccak256!("Proposed(uint32,uint32)");

pub fn log_proposed(address: impl Into<H160>, dao_id: DaoId, proposal_index: ProposalIndex) -> Log {
	log3(
		address.into(),
		SELECTOR_LOG_PROPOSED,
		H256::from_slice(&EvmDataWriter::new().write(dao_id).build()),
		H256::from_slice(&EvmDataWriter::new().write(proposal_index).build()),
		Vec::new(),
	)
}

/// A precompile to wrap the functionality from pallet-dao-treasury.
pub struct DaoTreasuryPrecompile<Runtime>(PhantomData<Runtime>);

#[precompile_utils::precompile]
impl<Runtime> DaoTreasuryPrecompile<Runtime>
where
	Runtime: pallet_dao_treasury::Config + pallet_evm::Config,
	Runtime::RuntimeCall: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo,
	<Runtime::RuntimeCall as Dispatchable>::RuntimeOrigin: From<Option<Runtime::AccountId>>,
	Runtime::RuntimeCall: From<pallet_dao_treasury::Call<Runtime>>,
	H256: From<<Runtime as frame_system::Config>::Hash>
		+ Into<<Runtime as frame_system::Config>::Hash>,
{
	/// Propose Treasury Spend
	/// The dispatch origin for this call must be Signed.
	///
	/// Parameters:
	/// * dao_id: DAO ID
	/// * value: Balance amount to be spent
	/// * beneficiary: Account to transfer balance to
	#[precompile::public("proposeSpend(uint32,uint128,address)")]
	fn propose_spend(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		value: u128,
		beneficiary: Address,
	) -> EvmResult {
		let origin = Runtime::AddressMapping::into_account_id(handle.context().caller);

		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		let proposal_index = pallet_dao_treasury::Pallet::<Runtime>::proposal_count(dao_id);

		let call = pallet_dao_treasury::Call::<Runtime>::propose_spend {
			dao_id,
			value: pallet_dao_treasury::Pallet::<Runtime>::u128_to_balance_of(value),
			beneficiary: Runtime::Lookup::unlookup(Runtime::AddressMapping::into_account_id(
				beneficiary.into(),
			)),
		};

		<RuntimeHelper<Runtime>>::try_dispatch(handle, Some(origin).into(), call)?;

		let log = log_proposed(handle.context().address, dao_id, proposal_index);

		handle.record_log_costs(&[&log])?;
		log.record(handle)?;

		Ok(())
	}

	#[precompile::public("proposalCount(uint32)")]
	#[precompile::view]
	fn proposal_count(handle: &mut impl PrecompileHandle, dao_id: DaoId) -> EvmResult<u32> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		let count = pallet_dao_treasury::Pallet::<Runtime>::proposal_count(dao_id);

		Ok(count)
	}

	#[precompile::public("approvals(uint32)")]
	#[precompile::view]
	fn approvals(handle: &mut impl PrecompileHandle, dao_id: DaoId) -> EvmResult<Vec<u32>> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		let approvals =
			pallet_dao_treasury::Pallet::<Runtime>::approvals(dao_id).into_iter().collect();

		Ok(approvals)
	}
}
