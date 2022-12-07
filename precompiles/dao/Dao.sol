// SPDX-License-Identifier: GPL-3.0-only
pragma solidity >=0.8.0;

/// @title The interface through which solidity contracts will interact with Societal Blockchain
/// We follow this same interface including four-byte function selectors, in the precompile that
/// wraps the pallet
interface PalletDao {

    /// @dev Create DAO .
    /// @param data DAO spec
    function create_dao(bytes memory data) external;
}
