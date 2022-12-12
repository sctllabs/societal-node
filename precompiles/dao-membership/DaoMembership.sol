// SPDX-License-Identifier: GPL-3.0-only
pragma solidity >=0.8.0;

/// @title The interface through which solidity contracts will interact with Societal Blockchain
/// We follow this same interface including four-byte function selectors, in the precompile that
/// wraps the pallet
interface PalletDaoMembership {
    /// @dev Check if the given account is a member of the membership.
    ///
    /// @param dao_id DAO ID.
    /// @param account Account to check membership.
    function isMember(uint32 dao_id, address account) external view returns (bool);
}
