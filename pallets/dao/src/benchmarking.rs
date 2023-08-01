//! Benchmarking setup for pallet-dao

use super::*;

use frame_benchmarking::{account, benchmarks, BenchmarkError};
use frame_support::traits::fungibles::Transfer;
use frame_system::RawOrigin;

use sp_runtime::traits::Bounded;

use crate::Pallet as DaoFactory;
use serde_json::{json, Value};

use codec::alloc::string::ToString;

const SEED: u32 = 0;
const COUNCIL_SEED: u32 = 1;
const TECH_COMMITTEE_SEED: u32 = 2;

// Create the pre-requisite information needed to create a dao.
fn setup_dao_payload<T: Config>(is_eth: bool, tier: Option<Value>) -> Vec<u8> {
	let mut dao_json = json!({
		"name": "name",
		"purpose": "purpose",
		"metadata": "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor \
			incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud \
			exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure \
			dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. \
			Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt \
			mollit anim id est laborum. Lorem ipsum dolor sit amet, consectetur adipiscing elit, \
			sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim \
			veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo \
			consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum do",
		"policy": {
			"proposal_period": 300000,
			"governance": {
				"GovernanceV1": {
					"enactment_period": 20,
					"launch_period": 200,
					"voting_period": 200,
					"vote_locking_period": 20,
					"fast_track_voting_period": 300000,
					"cooloff_period": 30,
					"minimum_deposit": 1,
					"external_origin": { "type": "AtLeast", "proportion": [ 1, 2 ] },
					"external_majority_origin": { "type": "AtLeast", "proportion": [ 1, 2 ] },
					"external_default_origin": { "type": "AtLeast", "proportion": [ 1, 2 ] },
					"fast_track_origin": { "type": "AtLeast", "proportion": [ 1, 2 ] },
					"instant_origin": { "type": "AtLeast", "proportion": [ 1, 2 ] },
					"instant_origin": { "type": "AtLeast", "proportion": [ 1, 2 ] },
					"instant_allowed": false,
					"cancellation_origin": { "type": "AtLeast", "proportion": [ 1, 2 ] },
					"blacklist_origin": { "type": "AtLeast", "proportion": [ 1, 2 ] },
					"cancel_proposal_origin": { "type": "AtLeast", "proportion": [ 1, 2 ] },
				  }
			}
		}
	});

	if is_eth {
		dao_json["token_address"] =
			Value::String("0x439ACbC2FAE8E4b9115e702AeBeAa9977621017C".to_string());
	} else {
		dao_json["token"] = json!({
			"token_id": 0,
			"initial_balance": "100000000",
			"metadata": {
				"name": "token",
				"symbol": "symbol",
				"decimals": 2
			}
		});
	}

	if tier.is_some() {
		dao_json["tier"] = tier.unwrap();
	}

	serde_json::to_vec(&dao_json).ok().unwrap()
}

fn setup_dao<T: Config>(is_eth: bool, tier: Option<Value>) -> Result<(), DispatchError> {
	let caller = account("caller", 0, SEED);
	T::Currency::make_free_balance_be(&caller, BalanceOf::<T>::max_value() / 2u32.into());
	let data = setup_dao_payload::<T>(is_eth, tier);

	DaoFactory::<T>::create_dao(RawOrigin::Signed(caller).into(), vec![], vec![], data)
}

fn get_dao_origin<T: Config>(dao_id: DaoId) -> Result<T::RuntimeOrigin, BenchmarkError> {
	let dao_account_id = DaoFactory::<T>::dao_account_id(dao_id);
	let dao_origin = DaoOrigin {
		dao_account_id: dao_account_id.clone(),
		proportion: DaoPolicyProportion::AtLeast((1, 1)),
	};

	T::ApproveOrigin::try_successful_origin(&dao_origin).map_err(|_| BenchmarkError::Weightless)
}

fn funded_account<T: Config>(
	name: &'static str,
	index: u32,
) -> Result<T::AccountId, DispatchError> {
	let caller: T::AccountId = account(name, index, SEED);
	// Give the account half of the maximum value of the `Balance` type.
	// Otherwise some transfers will fail with an overflow error.
	T::Currency::make_free_balance_be(&caller, BalanceOf::<T>::max_value() / 2u32.into());

	// Transferring some DAO tokens to the caller
	let dao_account_id = DaoFactory::<T>::dao_account_id(0);
	T::AssetProvider::transfer(0.into(), &dao_account_id, &caller, 10_000_u32.into(), true)?;

	Ok(caller)
}

benchmarks! {
	create_dao {
		let caller = account("caller", 0, SEED);

		let mut council = vec![];
		for index in 1..T::DaoMaxCouncilMembers::get() {
			council.push(account("account", index, COUNCIL_SEED));
		}

		let mut technical_committee = vec![];
		for index in 1..T::DaoMaxTechnicalCommitteeMembers::get() {
			technical_committee.push(account("account", index, TECH_COMMITTEE_SEED));
		}

		T::Currency::make_free_balance_be(&caller, BalanceOf::<T>::max_value() / 2u32.into());

		let data = setup_dao_payload::<T>(false, None);
	}: _(RawOrigin::Signed(caller), council, technical_committee, data)
	verify {
		assert_eq!(NextDaoId::<T>::get(), 1);
	}

	approve_dao {
		setup_dao::<T>(true, None)?;
		let dao_hash = PendingDaos::<T>::iter_keys().next().unwrap();
	}: _(RawOrigin::None, dao_hash, true)
	verify {
		assert_eq!(NextDaoId::<T>::get(), 1);
	}

	update_dao_metadata {
		setup_dao::<T>(false, None)?;
		let new_metadata = "Lorem Lorem dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor \
			incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud \
			exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure \
			dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. \
			Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt \
			mollit anim id est laborum. Lorem ipsum dolor sit amet, consectetur adipiscing elit, \
			sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim \
			veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo \
			consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum do";
		let origin = get_dao_origin::<T>(0)?;
	}: _<T::RuntimeOrigin>(origin, 0, new_metadata.as_bytes().to_vec())
	verify {
		let DaoConfig { metadata, .. } = Daos::<T>::get(0).unwrap().config;
		assert_eq!(metadata.to_vec(), new_metadata.as_bytes().to_vec());
	}

	update_dao_policy {
		setup_dao::<T>(false, None)?;
		let new_proposal_period = 500000;
		let policy = json!({
			"proposal_period": new_proposal_period,
			"approve_origin": { "type": "AtLeast", "proportion": [ 1, 2 ] },
			"spend_period": 130,
			"bounty_payout_delay": 100,
			"bounty_update_period": 100,
			"governance": {
			  "GovernanceV1": {
				"enactment_period": 20,
				"launch_period": 200,
				"voting_period": 200,
				"vote_locking_period": 20,
				"fast_track_voting_period": 300000,
				"cooloff_period": 30,
				"minimum_deposit": 1,
				"external_origin": { "type": "AtLeast", "proportion": [ 1, 2 ] },
				"external_majority_origin": { "type": "AtLeast", "proportion": [ 1, 2 ] },
				"external_default_origin": { "type": "AtLeast", "proportion": [ 1, 2 ] },
				"fast_track_origin": { "type": "AtLeast", "proportion": [ 1, 2 ] },
				"instant_origin": { "type": "AtLeast", "proportion": [ 1, 2 ] },
				"instant_origin": { "type": "AtLeast", "proportion": [ 1, 2 ] },
				"instant_allowed": false,
				"cancellation_origin": { "type": "AtLeast", "proportion": [ 1, 2 ] },
				"blacklist_origin": { "type": "AtLeast", "proportion": [ 1, 2 ] },
				"cancel_proposal_origin": { "type": "AtLeast", "proportion": [ 1, 2 ] },
			  }
			}
		});
		let origin = get_dao_origin::<T>(0)?;
	}: _<T::RuntimeOrigin>(origin, 0, serde_json::to_vec(&policy).ok().unwrap())
	verify {
		let DaoPolicy { proposal_period, .. } = Policies::<T>::get(0).unwrap();
		assert_eq!(proposal_period, new_proposal_period);
	}

	mint_dao_token {
		setup_dao::<T>(false, None)?;
		let origin = get_dao_origin::<T>(0)?;
	}: _<T::RuntimeOrigin>(origin, 0, 100_u32.into())
	verify {
		let dao_token_supply = T::AssetProvider::total_issuance(0.into());
		assert_eq!(dao_token_supply, 100000100_u32.into());
	}

	spend_dao_funds {
		setup_dao::<T>(false, None)?;
		let origin = get_dao_origin::<T>(0)?;
	}: _<T::RuntimeOrigin>(origin, 0)
	verify { }

	launch_dao_referendum {
		setup_dao::<T>(false, None)?;

		let proposer = funded_account::<T>("proposer", 0)?;
		let proposal = CallOf::<T>::from(Call::spend_dao_funds { dao_id: 0 });
		T::DaoReferendumBenchmarkHelper::propose(proposer, 0, proposal, 1_u32.into())?;

		let origin = get_dao_origin::<T>(0)?;
	}: _<T::RuntimeOrigin>(origin, 0)
	verify { }

	bake_dao_referendum {
		setup_dao::<T>(false, None)?;

		let proposer = funded_account::<T>("proposer", 0)?;
		let proposal = CallOf::<T>::from(Call::spend_dao_funds { dao_id: 0 });
		T::DaoReferendumBenchmarkHelper::propose(proposer, 0, proposal, 1_u32.into())?;

		let origin = get_dao_origin::<T>(0)?;
		DaoFactory::<T>::launch_dao_referendum(origin.clone(), 0)?;
	}: _<T::RuntimeOrigin>(origin, 0)
	verify { }

	subscribe {
		let tier = json!({
			"Default": "NoTier"
		});
		setup_dao::<T>(false, Some(tier))?;
		let origin = get_dao_origin::<T>(0)?;
		let dao_account_id = DaoFactory::<T>::dao_account_id(0);
		T::Currency::make_free_balance_be(&dao_account_id, BalanceOf::<T>::max_value() / 2u32.into());
	}: _<T::RuntimeOrigin>(origin, 0, None)
	verify { }

	extend_subscription {
		setup_dao::<T>(false, None)?;
		let origin = get_dao_origin::<T>(0)?;
		let dao_account_id = DaoFactory::<T>::dao_account_id(0);
		T::Currency::make_free_balance_be(&dao_account_id, BalanceOf::<T>::max_value() / 2u32.into());
	}: _<T::RuntimeOrigin>(origin, 0)
	verify { }

	change_subscription {
		setup_dao::<T>(false, None)?;
		let origin = get_dao_origin::<T>(0)?;
		let dao_account_id = DaoFactory::<T>::dao_account_id(0);
		T::Currency::make_free_balance_be(&dao_account_id, BalanceOf::<T>::max_value() / 2u32.into());
		T::DaoSubscriptionProvider::assign_subscription_tier(
			VersionedDaoSubscriptionTier::Default(DaoSubscriptionTierV1::Standard)
		)?;
	}: _<T::RuntimeOrigin>(origin, 0, VersionedDaoSubscriptionTier::Default(DaoSubscriptionTierV1::Standard))
	verify { }

	remove_dao {
		setup_dao::<T>(false, None)?;
	}: _<T::RuntimeOrigin>(RawOrigin::Root.into(), 0)
	verify { }

	impl_benchmark_test_suite!(DaoFactory, crate::mock::new_test_ext(), crate::mock::Test);
}
