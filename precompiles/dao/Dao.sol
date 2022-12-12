// SPDX-License-Identifier: GPL-3.0-only
pragma solidity >=0.8.0;

/// @title The interface through which solidity contracts will interact with Societal Blockchain
/// We follow this same interface including four-byte function selectors, in the precompile that
/// wraps the pallet
interface PalletDao {

    /// @dev Create DAO .
    /// @param data DAO spec
    function create_dao(address[] memory council, bytes memory data) external;

    /// @dev Get the DAO by ID.
    ///
    /// @param dao_id DAO ID.
    ///
    /// @custom:selector 55ef20e6
    function dao(uint32 dao_id) external view returns (bytes32[] memory dao);

    event DaoRegistered(address indexed who, uint32 dao_id);
}
