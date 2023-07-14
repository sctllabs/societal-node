use crate::*;
use frame_support::ensure;

use dao_primitives::{
	DaoSubscriptionDetailsV1, DaoTxValidityError, VersionedDaoSubscriptionDetails,
};
use frame_system::Config;
use scale_info::TypeInfo;
use sp_runtime::{
	traits::SignedExtension,
	transaction_validity::{InvalidTransaction, ValidTransaction},
};

/// Validate `attest` calls prior to execution. Needed to avoid a DoS attack since they are
/// otherwise free to place on chain.
#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct PrevalidateAttests<T: Config + Send + Sync>(sp_std::marker::PhantomData<T>);

impl<T: Config + Send + Sync> sp_std::fmt::Debug for PrevalidateAttests<T> {
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "DaoDemocracyPrevalidateAttests")
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
		// TODO: Move function call indexing inside extrinsic
		// Ensures the caller is a DAO member and indexes subscription function call
		let subscription = match call {
			RuntimeCall::DaoBounties(pallet_dao_bounties::Call::unassign_curator {
				dao_id,
				..
			}) |
			RuntimeCall::DaoBounties(pallet_dao_bounties::Call::accept_curator {
				dao_id, ..
			}) |
			RuntimeCall::DaoBounties(pallet_dao_bounties::Call::award_bounty {
				dao_id, ..
			}) |
			RuntimeCall::DaoBounties(pallet_dao_bounties::Call::claim_bounty {
				dao_id, ..
			}) |
			RuntimeCall::DaoBounties(pallet_dao_bounties::Call::extend_bounty_expiry {
				dao_id,
				..
			}) |
			RuntimeCall::DaoCouncil(pallet_dao_collective::Call::propose { dao_id, .. }) |
			RuntimeCall::DaoCouncil(pallet_dao_collective::Call::propose_with_meta {
				dao_id,
				..
			}) |
			RuntimeCall::DaoCouncil(pallet_dao_collective::Call::vote { dao_id, .. }) |
			RuntimeCall::DaoCouncil(pallet_dao_collective::Call::close { dao_id, .. }) |
			RuntimeCall::DaoTechnicalCommittee(pallet_dao_collective::Call::propose {
				dao_id,
				..
			}) |
			RuntimeCall::DaoTechnicalCommittee(
				pallet_dao_collective::Call::propose_with_meta { dao_id, .. },
			) |
			RuntimeCall::DaoTechnicalCommittee(pallet_dao_collective::Call::vote {
				dao_id,
				..
			}) |
			RuntimeCall::DaoTechnicalCommittee(pallet_dao_collective::Call::close {
				dao_id,
				..
			}) |
			RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::propose { dao_id, .. }) |
			RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::propose_with_meta {
				dao_id,
				..
			}) |
			RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::second { dao_id, .. }) |
			RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::vote { dao_id, .. }) |
			RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::delegate { dao_id, .. }) |
			RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::undelegate {
				dao_id, ..
			}) |
			RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::unlock { dao_id, .. }) |
			RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::remove_vote {
				dao_id, ..
			}) |
			RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::remove_other_vote {
				dao_id,
				..
			}) |
			RuntimeCall::DaoCouncilMembers(pallet_dao_membership::Call::change_key {
				dao_id,
				..
			}) |
			RuntimeCall::DaoTechnicalCommitteeMembers(
				pallet_dao_membership::Call::change_key { dao_id, .. },
			) => {
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

				DaoSubscription::do_ensure_limited(who, ACCOUNT_CALLS_PER_BLOCK).map_err(|_| {
					InvalidTransaction::Custom(DaoTxValidityError::AccountRateLimitExceeded.into())
				})?;

				let subscription = DaoSubscription::do_ensure_active(*dao_id).map_err(|_| {
					InvalidTransaction::Custom(DaoTxValidityError::DaoSubscriptionError.into())
				})?;

				let asset_accounts_total = Assets::accounts_total(dao_token);
				match asset_accounts_total {
					None => {},
					Some(accounts) => match subscription.clone().details {
						VersionedDaoSubscriptionDetails::Default(details) => match details {
							DaoSubscriptionDetailsV1 { max_members, .. } => {
								ensure!(
									accounts <= max_members,
									InvalidTransaction::Custom(
										DaoTxValidityError::TooManyDaoMembers.into()
									)
								)
							},
						},
					},
				}

				Some(subscription)
			},
			_ => None,
		};

		// Verifying the Subscription Tier
		match subscription {
			None => Ok(ValidTransaction::default()),
			Some(subscription) => {
				match subscription.details {
					VersionedDaoSubscriptionDetails::Default(details) => match details {
						DaoSubscriptionDetailsV1 {
							bounties,
							council,
							tech_committee,
							democracy,
							council_membership,
							tech_committee_membership,
							..
						} => match call {
							// Bounties
							RuntimeCall::DaoBounties(
								pallet_dao_bounties::Call::unassign_curator { .. },
							) => {
								ensure!(
									bounties.unassign_curator,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								)
							},
							RuntimeCall::DaoBounties(
								pallet_dao_bounties::Call::accept_curator { .. },
							) => {
								ensure!(
									bounties.accept_curator,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								)
							},
							RuntimeCall::DaoBounties(pallet_dao_bounties::Call::award_bounty {
								..
							}) => {
								ensure!(
									bounties.award_bounty,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								)
							},
							RuntimeCall::DaoBounties(pallet_dao_bounties::Call::claim_bounty {
								..
							}) => {
								ensure!(
									bounties.claim_bounty,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								)
							},
							RuntimeCall::DaoBounties(
								pallet_dao_bounties::Call::extend_bounty_expiry { .. },
							) => {
								ensure!(
									bounties.extend_bounty_expiry,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								)
							},
							RuntimeCall::DaoCouncil(pallet_dao_collective::Call::propose {
								proposal,
								..
							}) |
							RuntimeCall::DaoCouncil(
								pallet_dao_collective::Call::propose_with_meta { proposal, .. },
							) => {
								ensure!(
									council.enabled,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								);

								ensure!(
									council.propose,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								);

								// TODO: check dao pallet extrinsic calls?
								match **proposal {
									RuntimeCall::DaoBounties(
										pallet_dao_bounties::Call::accept_curator { .. },
									) => {
										ensure!(
											bounties.accept_curator,
											InvalidTransaction::Custom(
												DaoTxValidityError::Forbidden.into()
											)
										)
									},
									_ => {},
								}
							},
							RuntimeCall::DaoCouncil(pallet_dao_collective::Call::vote {
								..
							}) => {
								ensure!(
									council.vote,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								);
							},
							RuntimeCall::DaoCouncil(pallet_dao_collective::Call::close {
								..
							}) => {
								ensure!(
									council.close,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								);
							},
							RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::propose {
								..
							}) |
							RuntimeCall::DaoDemocracy(
								pallet_dao_democracy::Call::propose_with_meta { .. },
							) => {
								ensure!(
									democracy.propose,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								);
							},
							RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::second {
								..
							}) => {
								ensure!(
									democracy.second,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								);
							},
							RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::vote {
								..
							}) => {
								ensure!(
									democracy.vote,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								);
							},
							RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::delegate {
								..
							}) => {
								ensure!(
									democracy.delegate,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								);
							},
							RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::undelegate {
								..
							}) => {
								ensure!(
									democracy.delegate,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								);
							},
							RuntimeCall::DaoDemocracy(pallet_dao_democracy::Call::unlock {
								..
							}) => {
								ensure!(
									democracy.unlock,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								);
							},
							RuntimeCall::DaoDemocracy(
								pallet_dao_democracy::Call::remove_vote { .. },
							) => {
								ensure!(
									democracy.remove_vote,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								);
							},
							RuntimeCall::DaoDemocracy(
								pallet_dao_democracy::Call::remove_other_vote { .. },
							) => {
								ensure!(
									democracy.remove_other_vote,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								);
							},
							RuntimeCall::DaoTechnicalCommittee(
								pallet_dao_collective::Call::propose { .. },
							) => {
								ensure!(
									tech_committee.propose,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								);
							},
							RuntimeCall::DaoTechnicalCommittee(
								pallet_dao_collective::Call::propose_with_meta { .. },
							) => {
								ensure!(
									tech_committee.propose,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								);
							},
							RuntimeCall::DaoTechnicalCommittee(
								pallet_dao_collective::Call::vote { .. },
							) => {
								ensure!(
									tech_committee.vote,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								);
							},
							RuntimeCall::DaoTechnicalCommittee(
								pallet_dao_collective::Call::close { .. },
							) => {
								ensure!(
									tech_committee.close,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								);
							},
							RuntimeCall::DaoCouncilMembers(
								pallet_dao_membership::Call::change_key { .. },
							) => {
								ensure!(
									council_membership.change_key,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								);
							},
							RuntimeCall::DaoTechnicalCommitteeMembers(
								pallet_dao_membership::Call::change_key { .. },
							) => {
								ensure!(
									tech_committee_membership.change_key,
									InvalidTransaction::Custom(
										DaoTxValidityError::Forbidden.into()
									)
								);
							},
							_ => {},
						},
					},
				}

				Ok(ValidTransaction::default())
			},
		}
	}
}
