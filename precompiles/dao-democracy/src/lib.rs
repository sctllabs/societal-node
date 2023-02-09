//! Precompile to interact with pallet dao democracy through an evm precompile.

#![cfg_attr(not(feature = "std"), no_std)]

use fp_evm::PrecompileHandle;
use frame_support::{
	dispatch::{Dispatchable, GetDispatchInfo, PostDispatchInfo},
	traits::{Bounded, ConstU32, Currency},
};
use pallet_dao_democracy::{
	AccountVote, Call as DemocracyCall, Conviction, ReferendumInfo, Vote, VoteThreshold,
};
use pallet_evm::AddressMapping;
use pallet_preimage::Call as PreimageCall;
use precompile_utils::prelude::*;
use sp_core::{H160, H256, U256};
use sp_runtime::traits::StaticLookup;
use sp_std::{
	convert::{TryFrom, TryInto},
	fmt::Debug,
	marker::PhantomData,
};

#[cfg(feature = "dao_democracy_tests")]
#[cfg(test)]
mod mock;
#[cfg(test)]
#[cfg(feature = "dao_democracy_tests")]
mod tests;

/// Dao ID. Just a `u32`.
pub type DaoId = u32;

type GetProposalMetaLimit = ConstU32<100>;

type BalanceOf<Runtime> = <<Runtime as pallet_dao_democracy::Config>::Currency as Currency<
	<Runtime as frame_system::Config>::AccountId,
>>::Balance;

type DemocracyOf<Runtime> = pallet_dao_democracy::Pallet<Runtime>;

/// Solidity selector of the Proposed log, which is the Keccak of the Log signature.
pub const SELECTOR_LOG_PROPOSED: [u8; 32] = keccak256!("Proposed(uint32,uint32,uint256)");

/// Solidity selector of the Seconded log, which is the Keccak of the Log signature.
pub const SELECTOR_LOG_SECONDED: [u8; 32] = keccak256!("Seconded(uint32,uint32,address)");

/// Solidity selector of the StandardVote log, which is the Keccak of the Log signature.
pub const SELECTOR_LOG_STANDARD_VOTE: [u8; 32] =
	keccak256!("StandardVote(uint32,uint32,address,bool,uint256,uint8)");

/// Solidity selector of the Delegated log, which is the Keccak of the Log signature.
pub const SELECTOR_LOG_DELEGATED: [u8; 32] = keccak256!("Delegated(uint32,address,address)");

/// Solidity selector of the Undelegated log, which is the Keccak of the Log signature.
pub const SELECTOR_LOG_UNDELEGATED: [u8; 32] = keccak256!("Undelegated(uint32,address)");

/// A precompile to wrap the functionality from pallet democracy.
///
/// Grants evm-based DAOs the right to vote making them first-class citizens.
///
/// For an example of a political party that operates as a DAO, see PoliticalPartyDao.sol
pub struct DaoDemocracyPrecompile<Runtime>(PhantomData<Runtime>);

#[precompile_utils::precompile]
// #[precompile::test_concrete_types(mock::Runtime)]
impl<Runtime> DaoDemocracyPrecompile<Runtime>
where
	Runtime: pallet_dao_democracy::Config
		+ pallet_evm::Config
		+ frame_system::Config
		+ pallet_preimage::Config,
	BalanceOf<Runtime>: TryFrom<U256> + TryInto<u128> + Into<U256> + Debug + EvmData,
	Runtime::RuntimeCall: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo,
	<Runtime::RuntimeCall as Dispatchable>::RuntimeOrigin: From<Option<Runtime::AccountId>>,
	Runtime::RuntimeCall: From<DemocracyCall<Runtime>>,
	Runtime::RuntimeCall: From<PreimageCall<Runtime>>,
	Runtime::Hash: From<H256> + Into<H256>,
	Runtime::BlockNumber: Into<U256>,
{
	// The accessors are first. They directly return their result.
	#[precompile::public("publicPropCount(uint32)")]
	#[precompile::public("public_prop_count(uint32)")]
	#[precompile::view]
	fn public_prop_count(handle: &mut impl PrecompileHandle, dao_id: DaoId) -> EvmResult<U256> {
		// Fetch data from pallet
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let prop_count = DemocracyOf::<Runtime>::public_prop_count(dao_id);
		log::trace!(target: "democracy-precompile", "Prop count from pallet is {:?}", prop_count);

		Ok(prop_count.into())
	}

	#[precompile::public("depositOf(uint32,uint256)")]
	#[precompile::public("deposit_of(uint32,uint256)")]
	#[precompile::view]
	fn deposit_of(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		prop_index: SolidityConvert<U256, u32>,
	) -> EvmResult<U256> {
		let prop_index = prop_index.converted();

		// Fetch data from pallet
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let deposit = DemocracyOf::<Runtime>::deposit_of(dao_id, prop_index)
			.ok_or_else(|| revert("No such proposal in pallet democracy"))?
			.1;

		log::trace!(
			target: "democracy-precompile",
			"Deposit of dao {:?} proposal {:?} is {:?}", dao_id, prop_index, deposit
		);

		Ok(deposit.into())
	}

	#[precompile::public("lowestUnbaked(uint32)")]
	#[precompile::public("lowest_unbaked(uint32)")]
	#[precompile::view]
	fn lowest_unbaked(handle: &mut impl PrecompileHandle, dao_id: DaoId) -> EvmResult<U256> {
		// Fetch data from pallet
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let lowest_unbaked = DemocracyOf::<Runtime>::lowest_unbaked(dao_id);
		log::trace!(
			target: "democracy-precompile",
			"lowest unbaked dao {:?} referendum is {:?}", dao_id, lowest_unbaked
		);

		Ok(lowest_unbaked.into())
	}

	#[precompile::public("ongoingReferendumInfo(uint32,uint32)")]
	#[precompile::view]
	fn ongoing_referendum_info(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		ref_index: u32,
	) -> EvmResult<(U256, H256, u8, U256, U256, U256, U256)> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let ref_status = match DemocracyOf::<Runtime>::referendum_info(dao_id, ref_index) {
			Some(ReferendumInfo::Ongoing(ref_status)) => ref_status,
			Some(ReferendumInfo::Finished { .. }) => Err(revert("Referendum is finished"))?,
			None => Err(revert("Unknown referendum"))?,
		};

		let threshold_u8: u8 = match ref_status.threshold {
			VoteThreshold::SuperMajorityApprove => 0,
			VoteThreshold::SuperMajorityAgainst => 1,
			VoteThreshold::SimpleMajority => 2,
		};

		Ok((
			ref_status.end.into(),
			ref_status.proposal.hash(),
			threshold_u8,
			ref_status.delay.into(),
			ref_status.tally.ayes.into(),
			ref_status.tally.nays.into(),
			ref_status.tally.turnout.into(),
		))
	}

	#[precompile::public("finishedReferendumInfo(uint32,uint32)")]
	#[precompile::view]
	fn finished_referendum_info(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		ref_index: u32,
	) -> EvmResult<(bool, U256)> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let (approved, end) = match DemocracyOf::<Runtime>::referendum_info(dao_id, ref_index) {
			Some(ReferendumInfo::Ongoing(_)) => Err(revert("Referendum is ongoing"))?,
			Some(ReferendumInfo::Finished { approved, end }) => (approved, end),
			None => Err(revert("Unknown referendum"))?,
		};

		Ok((approved, end.into()))
	}

	// The dispatchable wrappers are next. They dispatch a Substrate inner Call.
	#[precompile::public("propose(uint32,bytes32,uint256)")]
	fn propose(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		proposal_hash: H256,
		value: U256,
	) -> EvmResult {
		Self::propose_with_meta(handle, dao_id, proposal_hash, value, BoundedBytes::from(""))
	}

	// The dispatchable wrappers are next. They dispatch a Substrate inner Call.
	#[precompile::public("propose(uint32,bytes32,uint256,bytes)")]
	fn propose_with_meta(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		proposal_hash: H256,
		value: U256,
		meta: BoundedBytes<GetProposalMetaLimit>,
	) -> EvmResult {
		handle.record_log_costs_manual(2, 32)?;

		// Fetch data from pallet
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;
		let prop_count = DemocracyOf::<Runtime>::public_prop_count(dao_id);

		let value = Self::u256_to_amount(value).in_field("value")?;

		log::trace!(
			target: "democracy-precompile",
			"Dao {:?}: Proposing with hash {:?}, and amount {:?}", dao_id, proposal_hash, value
		);

		// TODO: should have length
		let bounded = Bounded::Legacy::<pallet_dao_democracy::CallOf<Runtime>> {
			hash: proposal_hash,
			dummy: Default::default(),
		};

		let origin = Runtime::AddressMapping::into_account_id(handle.context().caller);
		let call = DemocracyCall::<Runtime>::propose_with_meta {
			dao_id,
			proposal: bounded,
			value,
			meta: Some(meta.into()),
		};

		RuntimeHelper::<Runtime>::try_dispatch(handle, Some(origin).into(), call)?;

		log3(
			handle.context().address,
			SELECTOR_LOG_PROPOSED,
			H256::from_slice(&EvmDataWriter::new().write(dao_id).build()),
			H256::from_low_u64_be(prop_count as u64), // proposal index,
			EvmDataWriter::new().write::<U256>(value.into()).build(),
		)
		.record(handle)?;

		Ok(())
	}

	#[precompile::public("second(uint32,uint256,uint256)")]
	fn second(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		prop_index: SolidityConvert<U256, u32>,
		seconds_upper_bound: SolidityConvert<U256, u32>,
	) -> EvmResult {
		handle.record_log_costs_manual(2, 32)?;
		let prop_index = prop_index.converted();
		let seconds_upper_bound = seconds_upper_bound.converted();

		log::trace!(
			target: "democracy-precompile",
			"Dao {:?}: Seconding proposal {:?}, with bound {:?}", dao_id, prop_index, seconds_upper_bound
		);

		let origin = Runtime::AddressMapping::into_account_id(handle.context().caller);
		let call = DemocracyCall::<Runtime>::second { dao_id, proposal: prop_index };

		RuntimeHelper::<Runtime>::try_dispatch(handle, Some(origin).into(), call)?;

		log3(
			handle.context().address,
			SELECTOR_LOG_SECONDED,
			H256::from_slice(&EvmDataWriter::new().write(dao_id).build()),
			H256::from_low_u64_be(prop_index as u64), // proposal index,
			EvmDataWriter::new().write::<Address>(handle.context().caller.into()).build(),
		)
		.record(handle)?;

		Ok(())
	}

	#[precompile::public("standardVote(uint32,uint256,bool,uint256,uint256)")]
	#[precompile::public("standard_vote(uint32,uint256,bool,uint256,uint256)")]
	fn standard_vote(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		ref_index: SolidityConvert<U256, u32>,
		aye: bool,
		vote_amount: U256,
		conviction: SolidityConvert<U256, u8>,
	) -> EvmResult {
		handle.record_log_costs_manual(2, 32 * 4)?;
		let ref_index = ref_index.converted();
		let vote_amount_balance = Self::u256_to_amount(vote_amount).in_field("voteAmount")?;

		let conviction_enum: Conviction = conviction.converted().try_into().map_err(|_| {
			RevertReason::custom("Must be an integer between 0 and 6 included")
				.in_field("conviction")
		})?;

		let vote = AccountVote::Standard {
			vote: Vote { aye, conviction: conviction_enum },
			balance: vote_amount_balance,
		};

		log::trace!(target: "democracy-precompile",
			"Dao {:?}: Voting {:?} on referendum #{:?}, with conviction {:?}",
			dao_id, aye, ref_index, conviction_enum
		);

		let origin = Runtime::AddressMapping::into_account_id(handle.context().caller);
		let call = DemocracyCall::<Runtime>::vote { dao_id, ref_index, vote };

		RuntimeHelper::<Runtime>::try_dispatch(handle, Some(origin).into(), call)?;

		log3(
			handle.context().address,
			SELECTOR_LOG_STANDARD_VOTE,
			H256::from_slice(&EvmDataWriter::new().write(dao_id).build()),
			H256::from_low_u64_be(ref_index as u64), // referendum index,
			EvmDataWriter::new()
				.write::<Address>(handle.context().caller.into())
				.write::<bool>(aye)
				.write::<U256>(vote_amount)
				.write::<u8>(conviction.converted())
				.build(),
		)
		.record(handle)?;

		Ok(())
	}

	#[precompile::public("removeVote(uint32,uint256)")]
	#[precompile::public("remove_vote(uint32,uint256)")]
	fn remove_vote(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		ref_index: SolidityConvert<U256, u32>,
	) -> EvmResult {
		let ref_index: u32 = ref_index.converted();

		log::trace!(
			target: "democracy-precompile",
			"Dao {:?}: Removing vote from referendum {:?}",
			dao_id,
			ref_index
		);

		let origin = Runtime::AddressMapping::into_account_id(handle.context().caller);
		let call = DemocracyCall::<Runtime>::remove_vote { dao_id, index: ref_index };

		RuntimeHelper::<Runtime>::try_dispatch(handle, Some(origin).into(), call)?;

		Ok(())
	}

	#[precompile::public("delegate(uint32,address,uint256,uint256)")]
	fn delegate(
		handle: &mut impl PrecompileHandle,
		dao_id: DaoId,
		representative: Address,
		conviction: SolidityConvert<U256, u8>,
		amount: U256,
	) -> EvmResult {
		handle.record_log_costs_manual(2, 32)?;
		let amount = Self::u256_to_amount(amount).in_field("amount")?;

		let conviction: Conviction = conviction.converted().try_into().map_err(|_| {
			RevertReason::custom("Must be an integer between 0 and 6 included")
				.in_field("conviction")
		})?;

		log::trace!(target: "democracy-precompile",
			"Dao {dao_id:?}: Delegating vote to {representative:?} with balance {amount:?} and conviction {conviction:?}",
		);

		let to = Runtime::AddressMapping::into_account_id(representative.into());
		let to: <Runtime::Lookup as StaticLookup>::Source = Runtime::Lookup::unlookup(to);
		let origin = Runtime::AddressMapping::into_account_id(handle.context().caller);
		let call = DemocracyCall::<Runtime>::delegate { dao_id, to, conviction, balance: amount };

		RuntimeHelper::<Runtime>::try_dispatch(handle, Some(origin).into(), call)?;

		log3(
			handle.context().address,
			SELECTOR_LOG_DELEGATED,
			H256::from_slice(&EvmDataWriter::new().write(dao_id).build()),
			handle.context().caller,
			EvmDataWriter::new().write::<Address>(representative).build(),
		)
		.record(handle)?;

		Ok(())
	}

	#[precompile::public("unDelegate(uint32)")]
	#[precompile::public("un_delegate(uint32)")]
	fn un_delegate(handle: &mut impl PrecompileHandle, dao_id: DaoId) -> EvmResult {
		handle.record_log_costs_manual(2, 0)?;
		let origin = Runtime::AddressMapping::into_account_id(handle.context().caller);
		let call = DemocracyCall::<Runtime>::undelegate { dao_id };

		RuntimeHelper::<Runtime>::try_dispatch(handle, Some(origin).into(), call)?;

		log4(
			handle.context().address,
			SELECTOR_LOG_UNDELEGATED,
			H256::from_slice(&EvmDataWriter::new().write(dao_id).build()),
			handle.context().caller,
			H256::from_slice(&EvmDataWriter::new().write(dao_id).build()),
			[],
		)
		.record(handle)?;

		Ok(())
	}

	#[precompile::public("unlock(uint32,address)")]
	fn unlock(handle: &mut impl PrecompileHandle, dao_id: DaoId, target: Address) -> EvmResult {
		let target: H160 = target.into();
		let target = Runtime::AddressMapping::into_account_id(target);
		let target: <Runtime::Lookup as StaticLookup>::Source = Runtime::Lookup::unlookup(target);

		log::trace!(
			target: "democracy-precompile",
			"Dao {:?}: Unlocking democracy tokens for {:?}", dao_id, target
		);

		let origin = Runtime::AddressMapping::into_account_id(handle.context().caller);
		let call = DemocracyCall::<Runtime>::unlock { dao_id, target };

		RuntimeHelper::<Runtime>::try_dispatch(handle, Some(origin).into(), call)?;

		Ok(())
	}

	fn u256_to_amount(value: U256) -> MayRevert<BalanceOf<Runtime>> {
		value
			.try_into()
			.map_err(|_| RevertReason::value_is_too_large("balance type").into())
	}
}
