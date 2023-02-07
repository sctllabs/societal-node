use crate::{DaoCouncilCollective, DaoCouncilMembership, DaoTechnicalCommitteeCollective};
use pallet_dao_collective_precompile::DaoCollectivePrecompile;
use pallet_dao_democracy_precompile::DaoDemocracyPrecompile;
use pallet_dao_eth_governance_precompile::DaoEthGovernancePrecompile;
use pallet_dao_membership_precompile::DaoMembershipPrecompile;
use pallet_dao_precompile::DaoPrecompile;
use pallet_dao_treasury_precompile::DaoTreasuryPrecompile;
use pallet_evm_precompile_blake2::Blake2F;
use pallet_evm_precompile_bn128::{Bn128Add, Bn128Mul, Bn128Pairing};
use pallet_evm_precompile_modexp::Modexp;
use pallet_evm_precompile_sha3fips::Sha3FIPS256;
use pallet_evm_precompile_simple::{ECRecover, ECRecoverPublicKey, Identity, Ripemd160, Sha256};
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
				PrecompileAt<AddressU64<6>, Bn128Add, ForbidRecursion, AllowDelegateCall>,
				PrecompileAt<AddressU64<7>, Bn128Mul, ForbidRecursion, AllowDelegateCall>,
				PrecompileAt<AddressU64<8>, Bn128Pairing, ForbidRecursion, AllowDelegateCall>,
				PrecompileAt<AddressU64<9>, Blake2F, ForbidRecursion, AllowDelegateCall>,
				// Non-Societal specific nor Ethereum precompiles:
				PrecompileAt<AddressU64<1024>, Sha3FIPS256>,
				PrecompileAt<AddressU64<1026>, ECRecoverPublicKey>,
				// Societal specific precompiles:
				PrecompileAt<AddressU64<2048>, DaoPrecompile<R>>,
				PrecompileAt<AddressU64<2049>, DaoTreasuryPrecompile<R>>,
				PrecompileAt<AddressU64<2050>, DaoCollectivePrecompile<R, DaoCouncilCollective>>,
				PrecompileAt<
					AddressU64<2051>,
					DaoCollectivePrecompile<R, DaoTechnicalCommitteeCollective>,
				>,
				PrecompileAt<AddressU64<2052>, DaoMembershipPrecompile<R, DaoCouncilMembership>>,
				PrecompileAt<AddressU64<2053>, DaoDemocracyPrecompile<R>>,
				PrecompileAt<AddressU64<2054>, DaoEthGovernancePrecompile<R>>,
			),
		>,
	),
>;
