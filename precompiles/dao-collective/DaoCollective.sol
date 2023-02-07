// SPDX-License-Identifier: GPL-3.0-only
pragma solidity >=0.8.0;

/// @title The interface through which solidity contracts will interact with Societal Blockchain
/// We follow this same interface including four-byte function selectors, in the precompile that
/// wraps the pallet
interface PalletDaoCollective {

    /// @dev Execute a proposal as a single member of the collective.
    /// The sender must be a member of the collective.
    /// This will NOT revert if the Substrate proposal is dispatched but fails !
    ///
    /// @param dao_id DAO ID.
    /// @param proposal SCALE-encoded Substrate call.
    function execute(uint32 dao_id, bytes memory proposal) external;

    /// @dev Make a proposal for a call.
    /// The sender must be a member of the collective.
    /// If the threshold is less than 2 then the proposal will be dispatched
    /// directly from the group of one member of the collective.
    ///
    /// @param dao_id DAO ID
    /// @param proposal SCALE-encoded Substrate call.
    /// @return index Index of the new proposal. Meaningless if threshold < 2
    function propose(uint32 dao_id, bytes memory proposal)
        external
        returns (uint32 index);

    /// @dev Same as 'propose' with additional 'meta' included.
    ///
    /// @param dao_id DAO ID
    /// @param proposal SCALE-encoded Substrate call.
    /// @param meta Proposal Metadata.
    /// @return index Index of the new proposal.
    function propose_with_meta(uint32 dao_id, bytes memory proposal, bytes memory meta)
        external
        returns (uint32 index);

    /// @dev Vote for a proposal.
    /// The sender must be a member of the collective.
    ///
    /// @param dao_id DAO ID.
    /// @param proposalHash Hash of the proposal to vote for. Ensure the caller knows what they're
    /// voting in case of front-running or reorgs.
    /// @param proposalIndex Index of the proposal (returned by propose).
    /// @param approve The vote itself, is the caller approving or not the proposal.
    function vote(
        uint32 dao_id,
        bytes32 proposalHash,
        uint32 proposalIndex,
        bool approve
    ) external;

    /// @dev Close a proposal.
    /// Can be called by anyone once there is enough votes.
    /// Reverts if called at a non appropriate time.
    ///
    /// @param dao_id DAO ID.
    /// @param proposalHash Hash of the proposal to close.
    /// @param proposalIndex Index of the proposal.
    /// @param proposalWeightBound Maximum amount of Substrate weight the proposal can use.
    /// This call will revert if the proposal call would use more.
    /// @param lengthBound Must be a value higher or equal to the length of the SCALE-encoded
    /// proposal in bytes.
    /// @return executed Was the proposal executed or removed?
    function close(
        uint32 dao_id,
        bytes32 proposalHash,
        uint32 proposalIndex,
        uint64 proposalWeightBound,
        uint32 lengthBound
    ) external returns (bool executed);

    /// @dev Compute the hash of a proposal.
    ///
    /// @param proposal SCALE-encoded Substrate call.
    /// @return proposalHash Hash of the proposal.
    function proposalHash(bytes memory proposal)
        external
        view
        returns (bytes32 proposalHash);

    /// @dev Get the hashes of active proposals.
    ///
    /// @param dao_id DAO ID.
    /// @return proposalsHash Hashes of active proposals.
    function proposals(uint32 dao_id) external view returns (bytes32[] memory proposalsHash);

    /// @dev Get the list of members.
    ///
    /// @param dao_id DAO ID.
    /// @return members List of members.
    function members(uint32 dao_id) external view returns (address[] memory members);

    /// @dev Check if the given account is a member of the collective.
    ///
    /// @param dao_id DAO ID.
    /// @param account Account to check membership.
    function isMember(uint32 dao_id, address account) external view returns (bool);

    event Executed(uint32 dao_id, bytes32 indexed proposalHash);
    event Proposed(
        address indexed who,
        uint32 dao_id,
        uint32 indexed proposalIndex,
        bytes32 indexed proposalHash
    );
    event Voted(address indexed who, uint32 dao_id, bytes32 indexed proposalHash, bool voted);
    event Closed(uint32 dao_id, bytes32 indexed proposalHash);
}
