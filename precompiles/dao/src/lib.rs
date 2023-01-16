#![cfg_attr(not(feature = "std"), no_std)]

extern crate core;

use fp_evm::{Log, PrecompileHandle};
use frame_support::dispatch::{Dispatchable, GetDispatchInfo, PostDispatchInfo};
use pallet_evm::AddressMapping;
use precompile_utils::{data::Address, prelude::*};
use sp_core::{ConstU32, H160, H256};
use sp_runtime::traits::StaticLookup;
use sp_std::{marker::PhantomData, prelude::*};

/// Dao ID. Just a `u32`.
pub type DaoId = u32;

pub const ENCODED_PROPOSAL_SIZE_LIMIT: u32 = 2u32.pow(16);
pub const ARRAY_LIMIT: u32 = 10u32;

type GetEncodedProposalSizeLimit = ConstU32<ENCODED_PROPOSAL_SIZE_LIMIT>;
type GetArrayLimit = ConstU32<ARRAY_LIMIT>;

/// Solidity selector of the DaoRegistered log.
pub const SELECTOR_LOG_DAO_REGISTERED: [u8; 32] = keccak256!("DaoRegistered(address,uint32)");

pub fn log_dao_registered(address: impl Into<H160>, dao_id: DaoId, who: impl Into<H160>) -> Log {
	log3(
		address.into(),
		SELECTOR_LOG_DAO_REGISTERED,
		who.into(),
		H256::from_slice(&EvmDataWriter::new().write(dao_id).build()),
		Vec::new(),
	)
}

/// A precompile to wrap the functionality from pallet-proxy.
pub struct DaoPrecompile<Runtime>(PhantomData<Runtime>);

#[precompile_utils::precompile]
impl<Runtime> DaoPrecompile<Runtime>
where
	Runtime: pallet_dao::Config + pallet_evm::Config,
	Runtime::RuntimeCall: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo,
	<Runtime::RuntimeCall as Dispatchable>::RuntimeOrigin: From<Option<Runtime::AccountId>>,
	Runtime::RuntimeCall: From<pallet_dao::Call<Runtime>>,
{
	/// The dispatch origin for this call must be Signed.
	///
	/// Parameters:
	/// * council: Set of accounts to be selected as DAO council
	/// * data: HEX encoded JSON DAO configuration
	#[precompile::public("createDao(address[],address[],bytes)")]
	fn create_dao(
		handle: &mut impl PrecompileHandle,
		council: BoundedVec<Address, GetArrayLimit>,
		technical_committee: BoundedVec<Address, GetArrayLimit>,
		data: BoundedBytes<GetEncodedProposalSizeLimit>,
	) -> EvmResult {
		let origin = Runtime::AddressMapping::into_account_id(handle.context().caller);

		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		let dao_id = pallet_dao::Pallet::<Runtime>::next_dao_id();

		let council = Vec::from(council)
			.into_iter()
			.map(|address| {
				Runtime::Lookup::unlookup(Runtime::AddressMapping::into_account_id(address.into()))
			})
			.collect();

		let technical_committee = Vec::from(technical_committee)
			.into_iter()
			.map(|address| {
				Runtime::Lookup::unlookup(Runtime::AddressMapping::into_account_id(address.into()))
			})
			.collect();

		let call = pallet_dao::Call::<Runtime>::create_dao {
			council,
			technical_committee,
			data: data.into(),
		};

		<RuntimeHelper<Runtime>>::try_dispatch(handle, Some(origin).into(), call)?;

		let log = log_dao_registered(handle.context().address, dao_id, handle.context().caller);

		handle.record_log_costs(&[&log])?;
		log.record(handle)?;

		Ok(())
	}
}
