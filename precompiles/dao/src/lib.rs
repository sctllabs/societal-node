#![cfg_attr(not(feature = "std"), no_std)]
#![feature(assert_matches)]

extern crate core;

use fp_evm::PrecompileHandle;
use frame_support::dispatch::{Dispatchable, GetDispatchInfo, PostDispatchInfo};
use pallet_evm::AddressMapping;
use precompile_utils::{data::Address, prelude::*};
use sp_core::ConstU32;
use sp_runtime::traits::StaticLookup;
use sp_std::{marker::PhantomData, prelude::*};

pub const ENCODED_PROPOSAL_SIZE_LIMIT: u32 = 2u32.pow(16);
pub const ARRAY_LIMIT: u32 = 10u32;

type GetEncodedProposalSizeLimit = ConstU32<ENCODED_PROPOSAL_SIZE_LIMIT>;
type GetArrayLimit = ConstU32<ARRAY_LIMIT>;

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
	#[precompile::public("create_dao(address[],bytes)")]
	fn create_dao(
		handle: &mut impl PrecompileHandle,
		council: BoundedVec<Address, GetArrayLimit>,
		data: BoundedBytes<GetEncodedProposalSizeLimit>,
	) -> EvmResult {
		let origin = Runtime::AddressMapping::into_account_id(handle.context().caller);

		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		let council = Vec::from(council)
			.into_iter()
			.map(|address| {
				Runtime::Lookup::unlookup(Runtime::AddressMapping::into_account_id(address.into()))
			})
			.collect();

		let call = pallet_dao::Call::<Runtime>::create_dao { council, data: data.into() };

		<RuntimeHelper<Runtime>>::try_dispatch(handle, Some(origin).into(), call)?;

		Ok(())
	}
}
