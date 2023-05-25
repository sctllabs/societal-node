use crate::{
	DaoCouncilCollective, DaoCouncilMembership, DaoTechnicalCommitteeCollective, LocalAssetInstance,
};
use frame_support::parameter_types;
use pallet_dao_bounties_precompile::DaoBountiesPrecompile;
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
use pallet_evm_precompileset_assets_erc20::{Erc20AssetsPrecompileSet, IsLocal};
use precompile_utils::precompile_set::*;

// TODO
/// The asset precompile address prefix. Addresses that match against this prefix will be routed
/// to Erc20AssetsPrecompileSet being marked as foreign
pub const FOREIGN_ASSET_PRECOMPILE_ADDRESS_PREFIX: &[u8] = &[255u8; 4];
/// The asset precompile address prefix. Addresses that match against this prefix will be routed
/// to Erc20AssetsPrecompileSet being marked as local
pub const LOCAL_ASSET_PRECOMPILE_ADDRESS_PREFIX: &[u8] = &[255u8, 255u8, 255u8, 254u8];

parameter_types! {
	pub ForeignAssetPrefix: &'static [u8] = FOREIGN_ASSET_PRECOMPILE_ADDRESS_PREFIX;
	pub LocalAssetPrefix: &'static [u8] = LOCAL_ASSET_PRECOMPILE_ADDRESS_PREFIX;
}

type EthereumPrecompilesChecks = (AcceptDelegateCall, CallableByContract, CallableByPrecompile);

pub type FrontierPrecompiles<R> = PrecompileSetBuilder<
	R,
	(
		// Skip precompiles if out of range.
		PrecompilesInRangeInclusive<
			(AddressU64<1>, AddressU64<4095>),
			(
				// Ethereum precompiles:
				// We allow DELEGATECALL to stay compliant with Ethereum behavior.
				PrecompileAt<AddressU64<1>, ECRecover, EthereumPrecompilesChecks>,
				PrecompileAt<AddressU64<2>, Sha256, EthereumPrecompilesChecks>,
				PrecompileAt<AddressU64<3>, Ripemd160, EthereumPrecompilesChecks>,
				PrecompileAt<AddressU64<4>, Identity, EthereumPrecompilesChecks>,
				PrecompileAt<AddressU64<5>, Modexp, EthereumPrecompilesChecks>,
				PrecompileAt<AddressU64<6>, Bn128Add, EthereumPrecompilesChecks>,
				PrecompileAt<AddressU64<7>, Bn128Mul, EthereumPrecompilesChecks>,
				PrecompileAt<AddressU64<8>, Bn128Pairing, EthereumPrecompilesChecks>,
				PrecompileAt<AddressU64<9>, Blake2F, EthereumPrecompilesChecks>,
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
				PrecompileAt<AddressU64<2055>, DaoBountiesPrecompile<R>>,
			),
		>,
		// TODO: use foreign assets when xcm integrated
		// Prefixed precompile sets (XC20)
		PrecompileSetStartingWith<
			LocalAssetPrefix,
			Erc20AssetsPrecompileSet<R, IsLocal, LocalAssetInstance>,
		>,
	),
>;
