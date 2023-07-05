use crate::*;

use dao_primitives::DaoTxValidityError;
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
		match call {
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
				Dao::do_ensure_member(*dao_id, who).map_err(|_| {
					InvalidTransaction::Custom(DaoTxValidityError::NotDaoMember.into())
				})?;

				DaoSubscription::do_ensure_active(*dao_id).map_err(|_| {
					InvalidTransaction::Custom(DaoTxValidityError::DaoSubscriptionError.into())
				})?;
			},
			_ => {},
		}
		Ok(ValidTransaction::default())
	}
}
