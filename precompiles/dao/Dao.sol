// SPDX-License-Identifier: GPL-3.0-only
pragma solidity >=0.8.0;

/// @title The interface through which solidity contracts will interact with Societal Blockchain
/// We follow this same interface including four-byte function selectors, in the precompile that
/// wraps the pallet
interface PalletDao {

    /// @dev Create DAO
    /// @param council group
    /// @param technical_committee group
    /// @param data DAO spec
    function createDao(address[] memory council, address[] memory technical_committee, bytes memory data) external;

    event DaoRegistered(address indexed who, uint32 dao_id);
}
