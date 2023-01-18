use crate::{DaoCouncilCollective, DaoCouncilMembership};
use pallet_dao_collective_precompile::DaoCollectivePrecompile;
use pallet_dao_democracy_precompile::DaoDemocracyPrecompile;
use pallet_dao_membership_precompile::DaoMembershipPrecompile;
use pallet_dao_precompile::DaoPrecompile;
use pallet_dao_treasury_precompile::DaoTreasuryPrecompile;
use pallet_evm_precompile_modexp::Modexp;
use pallet_evm_precompile_simple::{ECRecover, Identity, Ripemd160, Sha256};
use precompile_utils::precompile_set::*;

pub type FrontierPrecompiles<R> = PrecompileSetBuilder<
	R,
	(
		// Skip precompiles if out of range.
		PrecompilesInRangeInclusive<
			(AddressU64<1>, AddressU64<4095>),
			(
				// Ethereum precompiles:
				// We allow DELEGATECALL to stay compliant with Ethereum behavior.
				PrecompileAt<AddressU64<1>, ECRecover, ForbidRecursion, AllowDelegateCall>,
				PrecompileAt<AddressU64<2>, Sha256, ForbidRecursion, AllowDelegateCall>,
				PrecompileAt<AddressU64<3>, Ripemd160, ForbidRecursion, AllowDelegateCall>,
				PrecompileAt<AddressU64<4>, Identity, ForbidRecursion, AllowDelegateCall>,
				PrecompileAt<AddressU64<5>, Modexp, ForbidRecursion, AllowDelegateCall>,
				PrecompileAt<AddressU64<6>, DaoPrecompile<R>>,
				PrecompileAt<AddressU64<7>, DaoTreasuryPrecompile<R>>,
				PrecompileAt<AddressU64<8>, DaoCollectivePrecompile<R, DaoCouncilCollective>>,
				PrecompileAt<AddressU64<9>, DaoMembershipPrecompile<R, DaoCouncilMembership>>,
				PrecompileAt<AddressU64<10>, DaoMembershipPrecompile<R, DaoCouncilMembership>>,
			),
		>,
	),
>;
