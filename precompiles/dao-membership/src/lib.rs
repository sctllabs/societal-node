#![cfg_attr(not(feature = "std"), no_std)]

extern crate core;

use fp_evm::{Log, PrecompileHandle};
use frame_support::{
	dispatch::{Dispatchable, GetDispatchInfo, Pays, PostDispatchInfo},
	sp_runtime::traits::Hash,
	weights::Weight,
};
use pallet_evm::AddressMapping;
use parity_scale_codec::Decode;
use precompile_utils::{helpers::hash, prelude::*};
use sp_core::{ConstU32, H160, H256};
use sp_runtime::traits::StaticLookup;
use sp_std::{boxed::Box, marker::PhantomData, vec::Vec};

/// Dao ID. Just a `u32`.
pub type DaoId = u32;

/// A precompile to wrap the functionality from pallet-dao-membership.
pub struct DaoMembershipPrecompile<Runtime, Instance: 'static>(PhantomData<(Runtime, Instance)>);

#[precompile_utils::precompile]
impl<Runtime, Instance> DaoMembershipPrecompile<Runtime, Instance>
where
	Instance: 'static,
	Runtime: pallet_dao_membership::Config<Instance> + pallet_evm::Config,
	Runtime::RuntimeCall: Dispatchable<PostInfo = PostDispatchInfo> + GetDispatchInfo + Decode,
	Runtime::RuntimeCall: From<pallet_dao_membership::Call<Runtime, Instance>>,
	<Runtime::RuntimeCall as Dispatchable>::RuntimeOrigin: From<Option<Runtime::AccountId>>,
	H256: From<<Runtime as frame_system::Config>::Hash>
		+ Into<<Runtime as frame_system::Config>::Hash>,
{
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
			pallet_dao_membership::Pallet::<Runtime, Instance>::is_member(dao_id, &account);

		Ok(is_member)
	}
}
