#![cfg_attr(not(feature = "std"), no_std)]

extern crate core;

use fp_evm::{Log, PrecompileHandle};
use frame_support::dispatch::{Dispatchable, GetDispatchInfo, PostDispatchInfo};
use pallet_evm::AddressMapping;
use parity_scale_codec::Decode;
use precompile_utils::prelude::*;
use sp_core::{H160, H256};
use sp_std::marker::PhantomData;

/// Dao ID. Just a `u32`.
pub type DaoId = u32;
/// Bounty ID. Just a `u32`
pub type BountyId = u32;

/// Solidity selector of the Curator Accepted log.
pub const SELECTOR_LOG_BOUNTY_CURATOR_ACCEPTED: [u8; 32] =
	keccak256!("BountyCuratorAccepted(uint32,uint32,address)");
/// Solidity selector of the Curator Unassigned log.
pub const SELECTOR_LOG_BOUNTY_CURATOR_UNASSIGNED: [u8; 32] =
	keccak256!("BountyCuratorUnassigned(uint32,uint32)");
/// Solidity selector of the Bounty Awarded log.
pub const SELECTOR_LOG_BOUNTY_AWARDED: [u8; 32] =
	keccak256!("BountyAwarded(uint32,uint32,address)");
/// Solidity selector of the Bounty Claimed log.
pub const SELECTOR_LOG_BOUNTY_CLAIMED: [u8; 32] =
	keccak256!("BountyAwarded(uint32,uint32,uint128,address)");

pub fn log_bounty_curator_accepted(
	address: impl Into<H160>,
	who: impl Into<H160>,
	dao_id: DaoId,
	bounty_id: BountyId,
) -> Log {
	log3(
		address.into(),
		SELECTOR_LOG_BOUNTY_CURATOR_ACCEPTED,
		who.into(),
		H256::from_slice(&solidity::encode_arguments(dao_id)),
		&*solidity::encode_arguments(bounty_id),
	)
}

pub fn log_bounty_curator_unassigned(
	address: impl Into<H160>,
	dao_id: DaoId,
	bounty_id: BountyId,
) -> Log {
	log2(
		address.into(),
		SELECTOR_LOG_BOUNTY_CURATOR_UNASSIGNED,
		H256::from_slice(&solidity::encode_arguments(dao_id)),
		&*solidity::encode_arguments(bounty_id),
	)
}

pub fn log_bounty_awarded(
	address: impl Into<H160>,
	who: impl Into<H160>,
	dao_id: DaoId,
	bounty_id: BountyId,
	beneficiary: Address,
) -> Log {
	log4(
		address.into(),
		SELECTOR_LOG_BOUNTY_AWARDED,
		who.into(),
		H256::from_slice(&solidity::encode_arguments(dao_id)),
		H256::from_slice(&solidity::encode_arguments(bounty_id)),
		&*solidity::encode_arguments(beneficiary),
	)
}

pub fn log_bounty_claimed(
	address: impl Into<H160>,
	who: impl Into<H160>,
	dao_id: DaoId,
	bounty_id: BountyId,
) -> Log {
	log3(
		address.into(),
		SELECTOR_LOG_BOUNTY_CLAIMED,
		who.into(),
		H256::from_slice(&solidity::encode_arguments(dao_id)),
		&*solidity::encode_arguments(bounty_id),
	)
}

/// A precompile to wrap the functionality from pallet-dao-bounties.
pub struct DaoBountiesPrecompile<Runtime>(PhantomData<Runtime>);

#[precompile_utils::precompile]
impl<Runtime> DaoBountiesPrecompile<Runtime>
where
	Runtime: pallet_dao_bounties::Config + pallet_evm::Config,
	<Runtime as frame_system::Config>::RuntimeCall:
		Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo + Decode,
	<Runtime as frame_system::Config>::RuntimeCall: From<pallet_dao_bounties::Call<Runtime>>,
	<<Runtime as frame_system::Config>::RuntimeCall as Dispatchable>::RuntimeOrigin:
		From<Option<Runtime::AccountId>>,
{
	#[precompile::public("acceptCurator(uint32,uint32)")]
	fn accept_curator(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		bounty_id: BountyId,
	) -> EvmResult<u32> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		let origin = Runtime::AddressMapping::into_account_id(handle.context().caller);
		let call = pallet_dao_bounties::Call::<Runtime>::accept_curator { dao_id, bounty_id };
		RuntimeHelper::<Runtime>::try_dispatch(handle, Some(origin).into(), call)?;

		let log = log_bounty_curator_accepted(
			handle.context().address,
			handle.context().caller,
			dao_id,
			bounty_id,
		);

		handle.record_log_costs(&[&log])?;
		log.record(handle)?;

		Ok(bounty_id)
	}

	#[precompile::public("unassignCurator(uint32,uint32)")]
	fn unassign_curator(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		bounty_id: BountyId,
	) -> EvmResult<u32> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		let origin = Runtime::AddressMapping::into_account_id(handle.context().caller);
		let call = pallet_dao_bounties::Call::<Runtime>::unassign_curator { dao_id, bounty_id };
		RuntimeHelper::<Runtime>::try_dispatch(handle, Some(origin).into(), call)?;

		let log = log_bounty_curator_unassigned(handle.context().address, dao_id, bounty_id);

		handle.record_log_costs(&[&log])?;
		log.record(handle)?;

		Ok(bounty_id)
	}

	#[precompile::public("awardBounty(uint32,uint32,address)")]
	fn award_bounty(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		bounty_id: BountyId,
		beneficiary: Address,
	) -> EvmResult<Address> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		let origin = Runtime::AddressMapping::into_account_id(handle.context().caller);
		let to = Runtime::AddressMapping::into_account_id(beneficiary.into());
		let call = pallet_dao_bounties::Call::<Runtime>::award_bounty {
			dao_id,
			bounty_id,
			beneficiary: to,
		};

		RuntimeHelper::<Runtime>::try_dispatch(handle, Some(origin).into(), call)?;

		let log = log_bounty_awarded(
			handle.context().address,
			handle.context().caller,
			dao_id,
			bounty_id,
			beneficiary,
		);

		handle.record_log_costs(&[&log])?;
		log.record(handle)?;

		Ok(beneficiary)
	}

	#[precompile::public("claimBounty(uint32,uint32)")]
	fn claim_bounty(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		bounty_id: BountyId,
	) -> EvmResult<u32> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		let origin = Runtime::AddressMapping::into_account_id(handle.context().caller);
		let call = pallet_dao_bounties::Call::<Runtime>::claim_bounty { dao_id, bounty_id };

		RuntimeHelper::<Runtime>::try_dispatch(handle, Some(origin).into(), call)?;

		let log = log_bounty_claimed(
			handle.context().address,
			handle.context().caller,
			dao_id,
			bounty_id,
		);

		handle.record_log_costs(&[&log])?;
		log.record(handle)?;

		Ok(bounty_id)
	}

	#[precompile::public("bountyCount(uint32)")]
	#[precompile::view]
	fn bounty_count(_handle: &mut impl PrecompileHandle, dao_id: DaoId) -> EvmResult<u32> {
		Ok(pallet_dao_bounties::Pallet::<Runtime>::bounty_count(dao_id))
	}
}
