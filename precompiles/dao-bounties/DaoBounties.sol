// SPDX-License-Identifier: GPL-3.0-only
pragma solidity >=0.8.0;

/// @title The interface through which solidity contracts will interact with Societal Blockchain
/// We follow this same interface including four-byte function selectors, in the precompile that
/// wraps the pallet
interface PalletDaoBounties {

    /// Accept the curator role for a bounty.
    /// A deposit will be reserved from curator and refund upon successful payout.
    ///
    /// May only be called by the curator.
    ///
    /// @param dao_id DAO ID.
    /// @param bounty_id Bounty ID.
    function acceptCurator(uint32 dao_id, uint32 bounty_id) external;

    /// Unassign curator from a bounty.
    ///
    /// May be called by the `ApproveOrigin`, curator or anyone if and only if the curator is "inactive".
    ///
    /// @param dao_id DAO ID.
    /// @param bounty_id Bounty ID.
    function unassignCurator(uint32 dao_id, uint32 bounty_id) external;

    /// Extend the expiry time of an active bounty.
    ///
    /// May only be called by the curator.
    ///
    /// @param dao_id DAO ID.
    /// @param bounty_id Bounty ID.
    function extendBounty(uint32 dao_id, uint32 bounty_id) external;

    /// Award bounty to a beneficiary account. The beneficiary will be able to claim the funds
    /// after a delay.
    ///
    /// May only be called by the curator.
    ///
    /// @param dao_id DAO ID.
    /// @param bounty_id Bounty ID.
    function awardBounty(uint32 dao_id, uint32 bounty_id, address beneficiary) external;

    /// Claim the payout from an awarded bounty after payout delay.
    ///
    /// May only be called by the beneficiary of the bounty.
    ///
    /// @param dao_id DAO ID.
    /// @param bounty_id Bounty ID.
    function claimBounty(uint32 dao_id, uint32 bounty_id) external;

    /// @dev Get the DAO bounty count.
    ///
    /// @param dao_id DAO ID.
    /// @return count Bounty count.
    function bountyCount(uint32 dao_id) external view returns (uint32 count);

    event BountyCuratorAccepted(uint32 dao_id, uint32 bounty_id, address curator);
    event BountyCuratorUnassigned(uint32 dao_id, uint32 bounty_id);
    event BountyExtended(uint32 dao_id, uint32 bounty_id);
    event BountyAwarded(uint32 dao_id, uint32 bounty_id, address beneficiary);
    event BountyClaimed(uint32 dao_id, uint32 bounty_id);
}
