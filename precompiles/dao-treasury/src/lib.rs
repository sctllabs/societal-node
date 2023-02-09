#![cfg_attr(not(feature = "std"), no_std)]

extern crate core;

use fp_evm::PrecompileHandle;
use frame_support::dispatch::{Dispatchable, GetDispatchInfo, PostDispatchInfo};
use precompile_utils::prelude::*;
use sp_core::H256;
use sp_std::{marker::PhantomData, vec::Vec};

/// Dao ID. Just a `u32`.
pub type DaoId = u32;

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
