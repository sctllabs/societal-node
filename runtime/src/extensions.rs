use crate::*;
use cumulus_primitives_core::relay_chain::Balance;
use frame_support::ensure;

use crate::dao_config::TokenBalancesLimit;

use dao_primitives::{
	AccountFnCallRateLimiter, BountiesSubscriptionDetailsV1, CollectiveSubscriptionDetailsV1,
	DaoPalletSubscriptionDetails, DaoPalletSubscriptionDetailsV1, DaoSubscription as Subscription,
	DaoSubscriptionDetails, DaoTxValidityError, DemocracySubscriptionDetailsV1,
	MembershipSubscriptionDetailsV1, TokenBalances, TreasurySubscriptionDetailsV1,
	VersionedDaoSubscriptionTier,
};
use frame_system::Config;
use pallet_dao_subscription::Error;
use scale_info::TypeInfo;
use sp_runtime::{
	traits::SignedExtension,
	transaction_validity::{InvalidTransaction, ValidTransaction},
};

type SubscriptionDetails = DaoSubscriptionDetails<
	BlockNumber,
	Balance,
	TokenBalances<AssetId, Balance, TokenBalancesLimit>,
>;

/// Validate `attest` calls prior to execution. Needed to avoid a DoS attack since they are
/// otherwise free to place on chain.
#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct PrevalidateAttests<T: Config + Send + Sync>(sp_std::marker::PhantomData<T>);

impl<T: Config + Send + Sync> sp_std::fmt::Debug for PrevalidateAttests<T> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "SocietalNodePrevalidateAttests")
	}

	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

impl<T: Config + Send + Sync> PrevalidateAttests<T> {
	/// Create new `SignedExtension` to check runtime version.
	pub fn new() -> Self {
		Self(sp_std::marker::PhantomData)
	}
}

impl<T: Config + Send + Sync> Default for PrevalidateAttests<T> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T: Config + Send + Sync> SignedExtension for PrevalidateAttests<T> {
	type AccountId = AccountId;
	type Call = RuntimeCall;
	type AdditionalSigned = ();
	type Pre = ();
	const IDENTIFIER: &'static str = "SocietalRuntimePrevalidateAttests";

	fn additional_signed(&self) -> Result<Self::AdditionalSigned, TransactionValidityError> {
		Ok(())
	}

	fn pre_dispatch(
		self,
		who: &Self::AccountId,
		call: &Self::Call,
		info: &DispatchInfoOf<Self::Call>,
		len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		self.validate(who, call, info, len).map(|_| ())
	}

	// <weight>
	// The weight of this logic is included in the `attest` dispatchable.
	// </weight>
	fn validate(
		&self,
		who: &Self::AccountId,
		call: &Self::Call,
		_info: &DispatchInfoOf<Self::Call>,
		_len: usize,
	) -> TransactionValidity {
		validate_subscription(who, call)
	}
}

/// Subscription Validation:
/// - ensures `who` is a DAO member
/// - ensures the `call` by `who` is rate limited by max calls per block for the given account
/// - ensures the subscription is `active` and indexes function call
/// - ensures the max amount of members(in fact - gov token holders) is under its limit
/// - ensures the subscription tier function is enabled
/// - ensures the function extracted from proposal(both council/democracy) is enabled
fn validate_subscription(who: &AccountId, call: &RuntimeCall) -> TransactionValidity {
	match call {
		RuntimeCall::Dao(pallet_dao::Call::update_dao_metadata { dao_id, .. }) |
		RuntimeCall::Dao(pallet_dao::Call::update_dao_policy { dao_id, .. }) |
		RuntimeCall::Dao(pallet_dao::Call::mint_dao_token { dao_id, .. }) |
		RuntimeCall::DaoBounties(pallet_dao_bounties::Call::unassign_curator {
			dao_id, ..
		}) |
		RuntimeCall::DaoBounties(pallet_dao_bounties::Call::accept_curator { dao_id, .. }) |
		RuntimeCall::DaoBounties(pallet_dao_bounties::Call::award_bounty { dao_id, .. }) |
		RuntimeCall::DaoBounties(pallet_dao_bounties::Call::claim_bounty { dao_id, .. }) |
		RuntimeCall::DaoBounties(pallet_dao_bounties::Call::extend_bounty_expiry {
			dao_id,
			..
		}) |
		RuntimeCall::DaoCouncil(pallet_dao_collective::Call::propose { dao_id, .. }) |
		RuntimeCall::DaoCouncil(pallet_dao_collective::Call::propose_with_meta {
			dao_id, ..
		}) |
		RuntimeCall::DaoCouncil(pallet_dao_collective::Call::vote { dao_id, .. }) |
		RuntimeCall::DaoCouncil(pallet_dao_collective::Call::close { dao_id, .. }) |
		RuntimeCall::DaoTechnicalCommittee(pallet_dao_collective::Call::propose {
			dao_id, ..
		}) |
		RuntimeCall::DaoTechnicalCommittee(pallet_dao_collective::Call::propose_with_meta {
			dao_id,
			..
		}) |
		RuntimeCall::DaoTechnicalCommittee(pallet_dao_collective::Call::vote {
			dao_id, ..
		}) |
		RuntimeCall::DaoTechnicalCommittee(pallet_dao_collective::Call::close {
			dao_id, ..
		}) |
		RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::propose { dao_id, .. }) |
		RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::propose_with_meta {
			dao_id, ..
		}) |
		RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::second { dao_id, .. }) |
		RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::vote { dao_id, .. }) |
		RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::delegate { dao_id, .. }) |
		RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::undelegate { dao_id, .. }) |
		RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::unlock { dao_id, .. }) |
		RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::remove_vote { dao_id, .. }) |
		RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::remove_other_vote {
			dao_id, ..
		}) |
		RuntimeCall::DaoCouncilMembers(pallet_dao_membership::Call::change_key {
			dao_id, ..
		}) |
		RuntimeCall::DaoTechnicalCommitteeMembers(pallet_dao_membership::Call::change_key {
			dao_id,
			..
		}) |
		RuntimeCall::DaoTreasury(pallet_dao_treasury::Call::spend { dao_id, .. }) |
		RuntimeCall::DaoTreasury(pallet_dao_treasury::Call::transfer_token { dao_id, .. }) => {
			let (_, dao_token) = Dao::do_ensure_member(*dao_id, who).map_or_else(
				|_| Err(InvalidTransaction::Custom(DaoTxValidityError::NotDaoMember.into())),
				|(is_member, dao_token)| {
					ensure!(
						is_member,
						InvalidTransaction::Custom(DaoTxValidityError::NotDaoMember.into())
					);

					Ok((is_member, dao_token))
				},
			)?;

			DaoSubscription::ensure_limited(who, ACCOUNT_CALLS_PER_BLOCK).map_err(|_| {
				InvalidTransaction::Custom(DaoTxValidityError::AccountRateLimitExceeded.into())
			})?;

			DaoSubscription::ensure_active(*dao_id, |subscription| {
				let asset_accounts_total = Assets::accounts_total(dao_token);
				match asset_accounts_total {
					None => {},
					Some(accounts) => {
						let DaoSubscriptionDetails { max_members, .. } =
							subscription.clone().details;

						ensure!(
							accounts <= max_members,
							pallet_dao_subscription::Error::<Runtime>::TooManyMembers
						);
					},
				}

				validate_subscription_tier(call, subscription.clone())
					.map_err(|_| Error::<Runtime>::FunctionDisabled)?;

				Ok(())
			})
			.map_err(|e| match e {
				Error::FunctionDisabled =>
					InvalidTransaction::Custom(DaoTxValidityError::FunctionDisabled.into()),
				Error::TooManyMembers =>
					InvalidTransaction::Custom(DaoTxValidityError::TooManyMembers.into()),
				_ => InvalidTransaction::Custom(DaoTxValidityError::DaoSubscriptionError.into()),
			})?;
		},
		_ => {},
	};

	Ok(ValidTransaction::default())
}

fn validate_subscription_tier(
	call: &RuntimeCall,
	subscription: Subscription<
		BlockNumber,
		VersionedDaoSubscriptionTier,
		SubscriptionDetails,
		AssetId,
	>,
) -> TransactionValidity {
	validate_dao_pallet(call, subscription.details.clone())?;

	validate_bounties(call, subscription.details.clone())?;

	validate_council(call, subscription.details.clone())?;

	validate_democracy(call, subscription.details.clone())?;

	validate_tech_committee(call, subscription.details.clone())?;

	validate_council_membership(call, subscription.details.clone())?;

	validate_tech_committee_membership(call, subscription.details.clone())?;

	validate_treasury(call, subscription.details)?;

	Ok(ValidTransaction::default())
}

fn validate_proposal(
	proposal: &RuntimeCall,
	details: SubscriptionDetails,
) -> Result<(), TransactionValidityError> {
	validate_dao_pallet(proposal, details.clone())?;

	validate_bounties(proposal, details.clone())?;

	validate_council_membership(proposal, details.clone())?;

	validate_tech_committee_membership(proposal, details.clone())?;

	validate_treasury(proposal, details)?;

	Ok(())
}

fn validate_dao_pallet(
	call: &RuntimeCall,
	details: SubscriptionDetails,
) -> Result<(), TransactionValidityError> {
	let error = InvalidTransaction::Custom(DaoTxValidityError::FunctionDisabled.into());

	let DaoPalletSubscriptionDetails { dao, .. } = details.pallet_details;
	if let RuntimeCall::Dao(..) = call {
		let DaoPalletSubscriptionDetailsV1 {
			update_dao_metadata,
			update_dao_policy,
			mint_dao_token,
		} = dao.ok_or(error)?;

		match call {
			RuntimeCall::Dao(pallet_dao::Call::update_dao_metadata { .. }) => {
				ensure!(update_dao_metadata, error)
			},
			RuntimeCall::Dao(pallet_dao::Call::update_dao_policy { .. }) => {
				ensure!(update_dao_policy, error)
			},
			RuntimeCall::Dao(pallet_dao::Call::mint_dao_token { .. }) => {
				ensure!(mint_dao_token, error)
			},
			_ => {},
		}
	}

	Ok(())
}

fn validate_bounties(
	call: &RuntimeCall,
	details: SubscriptionDetails,
) -> Result<(), TransactionValidityError> {
	let error = InvalidTransaction::Custom(DaoTxValidityError::FunctionDisabled.into());

	let DaoPalletSubscriptionDetails { bounties, .. } = details.pallet_details;

	if let RuntimeCall::DaoBounties(..) = call {
		let BountiesSubscriptionDetailsV1 {
			create_bounty,
			propose_curator,
			unassign_curator,
			accept_curator,
			award_bounty,
			claim_bounty,
			close_bounty,
			extend_bounty_expiry,
		} = bounties.ok_or(error)?;

		match call {
			RuntimeCall::DaoBounties(pallet_dao_bounties::Call::create_bounty { .. }) |
			RuntimeCall::DaoBounties(pallet_dao_bounties::Call::create_token_bounty { .. }) => {
				ensure!(create_bounty, error)
			},
			RuntimeCall::DaoBounties(pallet_dao_bounties::Call::propose_curator { .. }) => {
				ensure!(propose_curator, error)
			},
			RuntimeCall::DaoBounties(pallet_dao_bounties::Call::unassign_curator { .. }) => {
				ensure!(unassign_curator, error)
			},
			RuntimeCall::DaoBounties(pallet_dao_bounties::Call::accept_curator { .. }) => {
				ensure!(accept_curator, error)
			},
			RuntimeCall::DaoBounties(pallet_dao_bounties::Call::award_bounty { .. }) => {
				ensure!(award_bounty, error)
			},
			RuntimeCall::DaoBounties(pallet_dao_bounties::Call::claim_bounty { .. }) => {
				ensure!(claim_bounty, error)
			},
			RuntimeCall::DaoBounties(pallet_dao_bounties::Call::close_bounty { .. }) => {
				ensure!(close_bounty, error)
			},
			RuntimeCall::DaoBounties(pallet_dao_bounties::Call::extend_bounty_expiry {
				..
			}) => {
				ensure!(extend_bounty_expiry, error)
			},
			_ => {},
		}
	};

	Ok(())
}

fn validate_council(
	call: &RuntimeCall,
	details: SubscriptionDetails,
) -> Result<(), TransactionValidityError> {
	let error = InvalidTransaction::Custom(DaoTxValidityError::FunctionDisabled.into());

	let DaoPalletSubscriptionDetails { council, .. } = details.clone().pallet_details;

	if let RuntimeCall::DaoCouncil(..) = call {
		let CollectiveSubscriptionDetailsV1 { propose, vote, close } = council.ok_or(error)?;

		match call {
			RuntimeCall::DaoCouncil(pallet_dao_collective::Call::propose { proposal, .. }) |
			RuntimeCall::DaoCouncil(pallet_dao_collective::Call::propose_with_meta {
				proposal,
				..
			}) => {
				ensure!(propose, error);

				validate_proposal(proposal, details)?;
			},
			RuntimeCall::DaoCouncil(pallet_dao_collective::Call::vote { .. }) => {
				ensure!(vote, error)
			},
			RuntimeCall::DaoCouncil(pallet_dao_collective::Call::close { .. }) => {
				ensure!(close, error)
			},
			_ => {},
		}
	}

	Ok(())
}

fn validate_democracy(
	call: &RuntimeCall,
	details: SubscriptionDetails,
) -> Result<(), TransactionValidityError> {
	let error = InvalidTransaction::Custom(DaoTxValidityError::FunctionDisabled.into());

	let DaoPalletSubscriptionDetails { democracy, .. } = details.clone().pallet_details;

	if let RuntimeCall::DaoDemocracy(..) = call {
		let DemocracySubscriptionDetailsV1 {
			propose,
			second,
			vote,
			delegate,
			undelegate,
			unlock,
			remove_vote,
			remove_other_vote,
		} = democracy.ok_or(error)?;

		match call {
			RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::propose { proposal, .. }) |
			RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::propose_with_meta {
				proposal,
				..
			}) => {
				ensure!(propose, error);

				let (proposal_dispatch_data, _) = DaoDemocracy::extract_original_call(proposal)
					.map_err(|_| InvalidTransaction::Custom(DaoTxValidityError::Unknown.into()))?;

				validate_proposal(&proposal_dispatch_data, details)?;
			},
			RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::second { .. }) => {
				ensure!(second, error)
			},
			RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::vote { .. }) => {
				ensure!(vote, error)
			},
			RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::delegate { .. }) => {
				ensure!(delegate, error)
			},
			RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::undelegate { .. }) => {
				ensure!(undelegate, error)
			},
			RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::unlock { .. }) => {
				ensure!(unlock, error)
			},
			RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::remove_vote { .. }) => {
				ensure!(remove_vote, error)
			},
			RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::remove_other_vote { .. }) => {
				ensure!(remove_other_vote, error)
			},
			_ => {},
		}
	}

	Ok(())
}

fn validate_tech_committee(
	call: &RuntimeCall,
	details: SubscriptionDetails,
) -> Result<(), TransactionValidityError> {
	let error = InvalidTransaction::Custom(DaoTxValidityError::FunctionDisabled.into());

	let DaoPalletSubscriptionDetails { tech_committee, .. } = details.clone().pallet_details;

	if let RuntimeCall::DaoTechnicalCommittee(..) = call {
		let CollectiveSubscriptionDetailsV1 { propose, vote, close } =
			tech_committee.ok_or(error)?;

		match call {
			RuntimeCall::DaoTechnicalCommittee(pallet_dao_collective::Call::propose {
				proposal,
				..
			}) |
			RuntimeCall::DaoTechnicalCommittee(
				pallet_dao_collective::Call::propose_with_meta { proposal, .. },
			) => {
				ensure!(propose, error);

				validate_proposal(proposal, details)?;
			},
			RuntimeCall::DaoTechnicalCommittee(pallet_dao_collective::Call::vote { .. }) => {
				ensure!(vote, error)
			},
			RuntimeCall::DaoTechnicalCommittee(pallet_dao_collective::Call::close { .. }) => {
				ensure!(close, error)
			},
			_ => {},
		}
	}

	Ok(())
}

fn validate_council_membership(
	call: &RuntimeCall,
	details: SubscriptionDetails,
) -> Result<(), TransactionValidityError> {
	let error = InvalidTransaction::Custom(DaoTxValidityError::FunctionDisabled.into());

	let DaoPalletSubscriptionDetails { council_membership, .. } = details.pallet_details;

	if let RuntimeCall::DaoCouncilMembers(..) = call {
		let MembershipSubscriptionDetailsV1 { add_member, remove_member, swap_member, change_key } =
			council_membership.ok_or(error)?;

		match call {
			RuntimeCall::DaoCouncilMembers(pallet_dao_membership::Call::add_member { .. }) => {
				ensure!(add_member, error)
			},
			RuntimeCall::DaoCouncilMembers(pallet_dao_membership::Call::remove_member {
				..
			}) => {
				ensure!(remove_member, error)
			},
			RuntimeCall::DaoCouncilMembers(pallet_dao_membership::Call::swap_member { .. }) => {
				ensure!(swap_member, error)
			},
			RuntimeCall::DaoCouncilMembers(pallet_dao_membership::Call::change_key { .. }) => {
				ensure!(change_key, error)
			},
			_ => {},
		}
	}

	Ok(())
}

fn validate_tech_committee_membership(
	call: &RuntimeCall,
	details: SubscriptionDetails,
) -> Result<(), TransactionValidityError> {
	let error = InvalidTransaction::Custom(DaoTxValidityError::FunctionDisabled.into());

	let DaoPalletSubscriptionDetails { tech_committee_membership, .. } = details.pallet_details;

	if let RuntimeCall::DaoTechnicalCommitteeMembers(..) = call {
		let MembershipSubscriptionDetailsV1 { add_member, remove_member, swap_member, change_key } =
			tech_committee_membership.ok_or(error)?;

		match call {
			RuntimeCall::DaoTechnicalCommitteeMembers(
				pallet_dao_membership::Call::add_member { .. },
			) => {
				ensure!(add_member, error)
			},
			RuntimeCall::DaoTechnicalCommitteeMembers(
				pallet_dao_membership::Call::remove_member { .. },
			) => {
				ensure!(remove_member, error)
			},
			RuntimeCall::DaoTechnicalCommitteeMembers(
				pallet_dao_membership::Call::swap_member { .. },
			) => {
				ensure!(swap_member, error)
			},
			RuntimeCall::DaoTechnicalCommitteeMembers(
				pallet_dao_membership::Call::change_key { .. },
			) => {
				ensure!(change_key, error)
			},
			_ => {},
		}
	}

	Ok(())
}

fn validate_treasury(
	call: &RuntimeCall,
	details: SubscriptionDetails,
) -> Result<(), TransactionValidityError> {
	let error = InvalidTransaction::Custom(DaoTxValidityError::FunctionDisabled.into());

	let DaoPalletSubscriptionDetails { treasury, .. } = details.pallet_details;

	if let RuntimeCall::DaoTreasury(..) = call {
		let TreasurySubscriptionDetailsV1 { spend, transfer_token } = treasury.ok_or(error)?;

		match call {
			RuntimeCall::DaoTreasury(pallet_dao_treasury::Call::spend { .. }) => {
				ensure!(spend, error)
			},
			RuntimeCall::DaoTreasury(pallet_dao_treasury::Call::transfer_token { .. }) => {
				ensure!(transfer_token, error)
			},
			_ => {},
		}
	}

	Ok(())
}
