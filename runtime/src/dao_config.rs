use super::*;
use frame_support::{parameter_types, PalletId};
use node_primitives::BlockNumber;
use pallet_dao::EnsureDao;
use pallet_dao_collective::EitherOfDiverseWithArg;
use sp_runtime::Permill;

parameter_types! {
	pub const DaoPalletId: PalletId = PalletId(*b"py/sctld");
	pub const DaoNameLimit: u32 = 20;
	pub const DaoStringLimit: u32 = 100;
	pub const DaoMetadataLimit: u32 = 750;
	pub const DaoMaxCouncilMembers: u32 = 100; // TODO
	pub const DaoMaxTechnicalCommitteeMembers: u32 = 100; // TODO
	pub const DaoMaxPendingItems: u32 = 100; // TODO
	pub const DaoMinTreasurySpendPeriod: u32 = 10; // TODO
	pub const TokenBalancesLimit: u32 = 10;
}

impl pallet_dao::Config for Runtime {
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type PalletId = DaoPalletId;
	type DaoNameLimit = DaoNameLimit;
	type DaoStringLimit = DaoStringLimit;
	type DaoMetadataLimit = DaoMetadataLimit;
	type DaoMaxCouncilMembers = DaoMaxCouncilMembers;
	type DaoMaxTechnicalCommitteeMembers = DaoMaxTechnicalCommitteeMembers;
	type DaoMaxPendingItems = DaoMaxPendingItems;
	type DaoMinTreasurySpendPeriod = DaoMinTreasurySpendPeriod;
	type TokenBalancesLimit = TokenBalancesLimit;
	type AssetId = AssetId;
	type Balance = Balance;
	type CouncilProvider = DaoCouncilMembers;
	type TechnicalCommitteeProvider = DaoTechnicalCommitteeMembers;
	type AssetProvider = Assets;
	type AuthorityId = pallet_dao::crypto::TestAuthId;
	type OffchainEthService = EthService<Dao>;
	type ApproveOrigin = EitherOfDiverseWithArg<
		EnsureDao<AccountId>,
		pallet_dao_collective::EnsureDaoOriginWithArg<AccountId, DaoCouncilCollective>,
	>;
	type Scheduler = Scheduler;
	type PalletsOrigin = OriginCaller;
	type Preimages = Preimage;
	type SpendDaoFunds = DaoTreasury;
	type DaoDemocracyProvider = DaoDemocracy;
	type DaoEthGovernanceProvider = DaoEthGovernance;
	type DaoBountiesProvider = DaoBounties;
	type DaoSubscriptionProvider = DaoSubscription;
	type WeightInfo = pallet_dao::weights::SubstrateWeight<Runtime>;

	#[cfg(feature = "runtime-benchmarks")]
	type DaoReferendumBenchmarkHelper = DaoDemocracy;
}

parameter_types! {
	pub const CouncilMotionDuration: BlockNumber = 5 * DAYS;
	pub const CouncilMaxProposals: u32 = 50;
	pub const CouncilMaxMembers: u32 = 100;
	pub const CollectiveMaxVotes: u32 = 100;
}

// TODO - Update settings
pub type DaoCouncilCollective = pallet_dao_collective::Instance1;
impl pallet_dao_collective::Config<DaoCouncilCollective> for Runtime {
	type RuntimeCall = RuntimeCall;
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type ProposalMetadataLimit = DaoMetadataLimit;
	type MaxProposals = CouncilMaxProposals;
	type MaxMembers = CouncilMaxMembers;
	type MaxVotes = CollectiveMaxVotes;
	type DefaultVote = pallet_dao_collective::MoreThanMajorityVote;
	type WeightInfo = pallet_dao_collective::weights::SubstrateWeight<Runtime>;
	type DaoProvider = Dao;
	type Preimages = Preimage;
	type Scheduler = Scheduler;
	type PalletsOrigin = OriginCaller;
}

parameter_types! {
	pub const TechnicalMotionDuration: BlockNumber = 5 * DAYS;
	pub const TechnicalMaxProposals: u32 = 100;
	pub const TechnicalMaxMembers: u32 = 100;
}

pub type DaoTechnicalCommitteeCollective = pallet_dao_collective::Instance2;
impl pallet_dao_collective::Config<DaoTechnicalCommitteeCollective> for Runtime {
	type RuntimeCall = RuntimeCall;
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type ProposalMetadataLimit = DaoMetadataLimit;
	type MaxProposals = TechnicalMaxProposals;
	type MaxMembers = TechnicalMaxMembers;
	type MaxVotes = CollectiveMaxVotes;
	type DefaultVote = pallet_dao_collective::MoreThanMajorityVote;
	type WeightInfo = pallet_dao_collective::weights::SubstrateWeight<Runtime>;
	type DaoProvider = Dao;
	type Preimages = Preimage;
	type Scheduler = Scheduler;
	type PalletsOrigin = OriginCaller;
}

parameter_types! {
	// pub const ProposalBond: Permill = Permill::from_percent(5);
	// pub const ProposalBondMinimum: Balance = DOLLARS;
	// pub const SpendPeriod: BlockNumber = MINUTES;
	pub const Burn: Permill = Permill::from_percent(50);
	pub const DaoTreasuryPalletId: PalletId = PalletId(*b"py/dtrsr");
	pub const MaxApprovals: u32 = 100;
}

impl pallet_dao_treasury::Config for Runtime {
	type PalletId = DaoTreasuryPalletId;
	type Currency = Balances;
	type AssetId = AssetId;
	type RuntimeEvent = RuntimeEvent;
	type OnSlash = ();
	type Burn = Burn;
	type BurnDestination = ();
	type SpendFunds = DaoBounties;
	type WeightInfo = pallet_dao_treasury::weights::SubstrateWeight<Runtime>;
	type MaxApprovals = MaxApprovals;
	type DaoProvider = Dao;
	type AssetProvider = Assets;
}

// TODO - Update settings
pub type DaoCouncilMembership = pallet_dao_membership::Instance1;
impl pallet_dao_membership::Config<DaoCouncilMembership> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MembershipInitialized = DaoCouncil;
	type MembershipChanged = DaoCouncil;
	type MembershipRemoved = DaoCouncil;
	type MaxMembers = TechnicalMaxMembers;
	type WeightInfo = pallet_dao_membership::weights::SubstrateWeight<Runtime>;
	type DaoProvider = Dao;
}

// TODO - Update settings
type DaoTechnicalCommitteeMembership = pallet_dao_membership::Instance2;
impl pallet_dao_membership::Config<DaoTechnicalCommitteeMembership> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MembershipInitialized = DaoTechnicalCommittee;
	type MembershipChanged = DaoTechnicalCommittee;
	type MembershipRemoved = DaoTechnicalCommittee;
	type MaxMembers = TechnicalMaxMembers;
	type WeightInfo = pallet_dao_membership::weights::SubstrateWeight<Runtime>;
	type DaoProvider = Dao;
}

parameter_types! {
	pub const MaxDaoDemocracyProposals: u32 = 50;
	pub const MaxDaoDemocracyVotes: u32 = 100;
	pub const MaxDaoDemocracyDeposits: u32 = 100;
	pub const MaxDaoDemocracyBlacklisted: u32 = 100;
}

impl pallet_dao_democracy::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type Assets = Assets;
	type Proposal = RuntimeCall;

	/// A straight majority of the council can decide what their next motion is.
	type ExternalOrigin = EitherOfDiverseWithArg<
		EnsureDao<AccountId>,
		pallet_dao_collective::EnsureDaoOriginWithArg<AccountId, DaoCouncilCollective>,
	>;
	/// A super-majority can have the next scheduled referendum be a straight majority-carries vote.
	type ExternalMajorityOrigin = EitherOfDiverseWithArg<
		EnsureDao<AccountId>,
		pallet_dao_collective::EnsureDaoOriginWithArg<AccountId, DaoCouncilCollective>,
	>;
	/// A unanimous council can have the next scheduled referendum be a straight default-carries
	/// (NTB) vote.
	type ExternalDefaultOrigin = EitherOfDiverseWithArg<
		EnsureDao<AccountId>,
		pallet_dao_collective::EnsureDaoOriginWithArg<AccountId, DaoCouncilCollective>,
	>;
	/// Two thirds of the technical committee can have an ExternalMajority/ExternalDefault vote
	/// be tabled immediately and with a shorter voting/enactment period.
	type FastTrackOrigin = EitherOfDiverseWithArg<
		EnsureDao<AccountId>,
		pallet_dao_collective::EnsureDaoOriginWithArg<AccountId, DaoCouncilCollective>,
	>;
	type InstantOrigin = EitherOfDiverseWithArg<
		EnsureDao<AccountId>,
		pallet_dao_collective::EnsureDaoOriginWithArg<AccountId, DaoCouncilCollective>,
	>;
	// To cancel a proposal which has been passed, 2/3 of the council must agree to it.
	type CancellationOrigin = EitherOfDiverseWithArg<
		EnsureDao<AccountId>,
		pallet_dao_collective::EnsureDaoOriginWithArg<AccountId, DaoCouncilCollective>,
	>;
	// To cancel a proposal before it has been passed, the technical committee must be unanimous or
	// Root must agree.
	type CancelProposalOrigin = EitherOfDiverseWithArg<
		EnsureDao<AccountId>,
		pallet_dao_collective::EnsureDaoOriginWithArg<AccountId, DaoCouncilCollective>,
	>;
	type BlacklistOrigin = EitherOfDiverseWithArg<
		EnsureDao<AccountId>,
		pallet_dao_collective::EnsureDaoOriginWithArg<AccountId, DaoCouncilCollective>,
	>;
	// Any single technical committee member may veto a coming council proposal, however they can
	// only do it once and it lasts only for the cool-off period.
	type VetoOrigin =
		pallet_dao_collective::EnsureMember<AccountId, DaoTechnicalCommitteeCollective>;
	type Slash = Treasury;
	type Scheduler = Scheduler;
	type Preimages = Preimage;
	type PalletsOrigin = OriginCaller;
	type MaxVotes = MaxDaoDemocracyVotes;
	type WeightInfo = pallet_dao_democracy::weights::SubstrateWeight<Runtime>;
	type MaxProposals = MaxDaoDemocracyProposals;
	type MaxDeposits = MaxDaoDemocracyDeposits;
	type MaxBlacklisted = MaxDaoDemocracyBlacklisted;
	type ProposalMetadataLimit = DaoMetadataLimit;
	type DaoProvider = Dao;
}

parameter_types! {
	pub const EthGovernanceMaxProposals: u32 = 50;
	pub const EthGovernanceMaxPendingProposals: u32 = 50;
	pub const EthGovernanceMaxVotes: u32 = 50;
	pub const EthGovernanceMaxPendingVotes: u32 = 50;
}

impl pallet_dao_eth_governance::Config for Runtime {
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type Balance = Balance;
	type Proposal = RuntimeCall;
	type ProposalMetadataLimit = DaoMetadataLimit;
	type EthRpcUrlLimit = DaoStringLimit;
	type MaxProposals = EthGovernanceMaxProposals;
	type MaxPendingProposals = EthGovernanceMaxPendingProposals;
	type MaxVotes = EthGovernanceMaxVotes;
	type MaxPendingVotes = EthGovernanceMaxPendingVotes;
	type DaoProvider = Dao;
	type Preimages = Preimage;
	type AuthorityId = pallet_dao::crypto::TestAuthId;
	type OffchainEthService = EthService<DaoEthGovernance>;
	type Scheduler = Scheduler;
	type PalletsOrigin = OriginCaller;
	type WeightInfo = pallet_dao_eth_governance::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
	pub const DaoBountiesMaximumReasonLength: u32 = 100;
}

impl pallet_dao_bounties::Config for Runtime {
	type Assets = Assets;
	type RuntimeEvent = RuntimeEvent;
	type MaximumReasonLength = DaoBountiesMaximumReasonLength;
	type WeightInfo = pallet_dao_bounties::weights::SubstrateWeight<Runtime>;
	type ChildBountyManager = ();
}

impl pallet_dao_subscription::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type AssetId = AssetId;
	type TreasuryPalletId = TreasuryPalletId;
	type TokenBalancesLimit = TokenBalancesLimit;
	type AssetProvider = Assets;
	type WeightInfo = pallet_dao_subscription::weights::SubstrateWeight<Runtime>;
}
