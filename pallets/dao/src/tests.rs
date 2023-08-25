use crate::{mock::*, BoundedVec, Config, Dao, DaoConfig, DaoPolicy, Error};
use codec::Compact;
use dao_primitives::{
	BountyPayoutDelay, BountyUpdatePeriod, DaoPolicyProportion, DaoStatus, DaoToken,
	TreasurySpendPeriod,
};
use frame_support::{
	assert_noop, assert_ok,
	traits::tokens::fungibles::{metadata::Inspect as MetadataInspect, Inspect},
};
use pallet_balances::Error as BalancesError;
use pallet_dao_democracy::Error as DemocracyError;
use pallet_dao_subscription::Error as SubscriptionError;
use serde_json::{json, Value};
use sp_core::{crypto::Ss58Codec, sr25519::Public};

use super::*;

#[test]
fn create_dao_invalid_input() {
	new_test_ext().execute_with(|| {
		let account = Public::from_string("/Alice").ok().unwrap();
		let account1 = Public::from_string("/Bob").ok().unwrap();

		assert_noop!(
			DaoFactory::create_dao(
				RuntimeOrigin::signed(account.clone()),
				vec![account, account1],
				vec![],
				r"invalid input".as_bytes().to_vec()
			),
			Error::<Test>::InvalidInput
		);
	})
}

#[test]
fn create_dao_fails_on_string_limits() {
	new_test_ext().execute_with(|| {
		let account = Public::from_string("/Alice").ok().unwrap();
		let account1 = Public::from_string("/Bob").ok().unwrap();

		let mut dao_json = get_dao_json();

		dao_json["name"] = Value::String("very long name above the limits".to_string());
		assert_noop!(
			DaoFactory::create_dao(
				RuntimeOrigin::signed(account.clone()),
				vec![account.clone(), account1.clone()],
				vec![],
				serde_json::to_vec(&dao_json).ok().unwrap()
			),
			Error::<Test>::NameTooLong
		);

		dao_json = get_dao_json();
		let new_purpose = "very long purpose above the limits very long purpose above the \
			limits very long purpose above the limits";
		dao_json["purpose"] = Value::String(new_purpose.to_string());
		assert_noop!(
			DaoFactory::create_dao(
				RuntimeOrigin::signed(account.clone()),
				vec![account.clone(), account1.clone()],
				vec![],
				serde_json::to_vec(&dao_json).ok().unwrap()
			),
			Error::<Test>::PurposeTooLong
		);

		dao_json = get_dao_json();
		let new_metadata = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor \
			incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud \
			exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure \
			dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. \
			Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt \
			mollit anim id est laborum. Lorem ipsum dolor sit amet, consectetur adipiscing elit, \
			sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim \
			veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo \
			consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.";
		dao_json["metadata"] = Value::String(new_metadata.to_string());
		assert_noop!(
			DaoFactory::create_dao(
				RuntimeOrigin::signed(account.clone()),
				vec![account, account1.clone()],
				vec![],
				serde_json::to_vec(&dao_json).ok().unwrap()
			),
			Error::<Test>::MetadataTooLong
		);
	});
}

#[test]
fn create_dao_token_failure() {
	new_test_ext().execute_with(|| {
		let account = Public::from_string("/Alice").ok().unwrap();
		let account1 = Public::from_string("/Bob").ok().unwrap();

		let mut dao_json = get_dao_json();

		dao_json["token"] = Value::Null;
		dao_json["token_id"] = Value::Null;
		assert_noop!(
			DaoFactory::create_dao(
				RuntimeOrigin::signed(account.clone()),
				vec![account.clone(), account1.clone()],
				vec![],
				serde_json::to_vec(&dao_json).ok().unwrap()
			),
			Error::<Test>::TokenNotProvided
		);

		dao_json = get_dao_json();
		dao_json["token"] = Value::Null;
		dao_json["token_id"] = json!(1);
		assert_noop!(
			DaoFactory::create_dao(
				RuntimeOrigin::signed(account.clone()),
				vec![account.clone(), account1.clone()],
				vec![],
				serde_json::to_vec(&dao_json).ok().unwrap()
			),
			Error::<Test>::TokenNotExists
		);
	})
}

#[test]
fn create_dao_token_balance_invalid() {
	new_test_ext().execute_with(|| {
		let account = Public::from_string("/Alice").ok().unwrap();
		Balances::make_free_balance_be(&account, 1_000_000_000_000_000_000);
		let account1 = Public::from_string("/Bob").ok().unwrap();

		let mut dao_json = get_dao_json();

		dao_json["token"] = json!({
			"token_id": 1,
			"initial_balance": "0",
			"min_balance": "0",
			"metadata": {
				"name": "dao",
				"symbol": "daot",
				"decimals": 2
			}
		});

		assert_noop!(
			DaoFactory::create_dao(
				RuntimeOrigin::signed(account.clone()),
				vec![account.clone(), account1.clone()],
				vec![],
				serde_json::to_vec(&dao_json).ok().unwrap()
			),
			Error::<Test>::TokenBalanceInvalid
		);
	})
}

#[test]
fn create_dao_token_already_exists() {
	new_test_ext().execute_with(|| {
		let account = Public::from_string("/Alice").ok().unwrap();
		Balances::make_free_balance_be(&account, 1_000_000_000_000_000_000);

		assert_ok!(Assets::create(
			RuntimeOrigin::signed(account.clone()),
			Compact(0),
			account.into(),
			1
		));
		assert_ok!(Assets::mint(
			RuntimeOrigin::signed(account.clone()),
			Compact(0),
			account.into(),
			1
		));

		let dao_json = get_dao_json();

		assert_noop!(
			DaoFactory::create_dao(
				RuntimeOrigin::signed(account.clone()),
				vec![account.clone()],
				vec![],
				serde_json::to_vec(&dao_json).ok().unwrap()
			),
			Error::<Test>::TokenAlreadyExists
		);
	})
}

#[test]
fn create_dao_works() {
	new_test_ext().execute_with(|| {
		create_dao();

		let account = Public::from_string("/Alice").ok().unwrap();
		let account1 = Public::from_string("/Bob").ok().unwrap();

		assert_eq!(DaoFactory::next_dao_id(), 1);

		assert_eq!(
			<Test as Config>::AssetProvider::symbol(TokenId::get()),
			TokenSymbol::get().as_bytes().to_vec()
		);
		assert_eq!(<Test as Config>::AssetProvider::decimals(TokenId::get()), TokenDecimals::get());
		assert_eq!(<Test as Config>::AssetProvider::minimum_balance(TokenId::get()), 1);

		assert_eq!(*Members::get().get(&0).unwrap(), vec![account, account1]);

		assert_eq!(DaoFactory::daos(0).is_some(), true);
		assert_eq!(
			DaoFactory::daos(0).unwrap(),
			Dao {
				founder: account,
				account_id: Public::from_string("5EYCAe5ijiYfqMFxyJDmHoxzF1VJ4NTsqtRAgdjN3q6pCz51")
					.ok()
					.unwrap(),
				config: DaoConfig {
					name: BoundedVec::<u8, <Test as Config>::DaoNameLimit>::try_from(
						DaoName::get().as_bytes().to_vec()
					)
					.unwrap(),
					purpose: BoundedVec::<u8, <Test as Config>::DaoStringLimit>::try_from(
						DaoPurpose::get().as_bytes().to_vec()
					)
					.unwrap(),
					metadata: BoundedVec::<u8, <Test as Config>::DaoMetadataLimit>::try_from(
						DaoMetadata::get().as_bytes().to_vec()
					)
					.unwrap(),
				},
				status: DaoStatus::Success,
				token: DaoToken::FungibleToken(TokenId::get()),
			}
		);

		assert_eq!(DaoFactory::policies(0).is_some(), true);
		assert_eq!(
			DaoFactory::policies(0).unwrap(),
			DaoPolicy {
				proposal_period: 100,
				approve_origin: DaoPolicyProportion::AtLeast((1, 2)),
				governance: Some(DaoGovernance::GovernanceV1(GovernanceV1Policy {
					enactment_period: 20,
					launch_period: 200,
					voting_period: 200,
					vote_locking_period: 20,
					fast_track_voting_period: 300000,
					cooloff_period: 30,
					minimum_deposit: 1,
					external_origin: Default::default(),
					external_majority_origin: Default::default(),
					external_default_origin: Default::default(),
					fast_track_origin: Default::default(),
					instant_origin: Default::default(),
					instant_allowed: false,
					cancellation_origin: Default::default(),
					blacklist_origin: Default::default(),
					cancel_proposal_origin: Default::default(),
				})),
				bounty_payout_delay: BountyPayoutDelay(7200),
				bounty_update_period: BountyUpdatePeriod(7200),
				spend_period: TreasurySpendPeriod(7200),
			}
		);
	});
}

#[test]
fn create_dao_eth_works() {
	let mut ext = new_test_ext();

	register_offchain_ext(&mut ext);

	ext.execute_with(|| {
		System::set_block_number(1);

		let account = Public::from_string("/Alice").ok().unwrap();
		Balances::make_free_balance_be(&account, 1_000_000_000_000_000_000);
		let account1 = Public::from_string("/Bob").ok().unwrap();

		let token_address = "0x439ACbC2FAE8E4b9115e702AeBeAa9977621017C";

		let mut dao_json = get_dao_json();
		dao_json["token"] = Value::Null;
		dao_json["token_id"] = Value::Null;
		dao_json["token_address"] = json!(token_address);

		assert_ok!(DaoFactory::create_dao(
			RuntimeOrigin::signed(account.clone()),
			vec![account.clone(), account1.clone()],
			vec![account.clone(), account1.clone()],
			serde_json::to_vec(&dao_json).ok().unwrap()
		));
	});
}

#[test]
fn remove_dao_works() {
	new_test_ext().execute_with(|| {
		create_dao();

		assert_ok!(DaoFactory::remove_dao(RuntimeOrigin::root(), 0));

		assert_eq!(DaoFactory::daos(0).is_none(), true);
		assert_eq!(DaoFactory::policies(0).is_none(), true);
	})
}

#[test]
fn ensures_member() {
	new_test_ext().execute_with(|| {
		create_dao();

		let account = Public::from_string("/Alice").ok().unwrap();

		assert_ok!(DaoFactory::do_ensure_member(0, &account));
	})
}

#[test]
fn update_dao_metadata_too_long() {
	new_test_ext().execute_with(|| {
		create_dao();

		let new_metadata =
			"Lorem Lorem dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor \
			incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud \
			exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure \
			dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. \
			Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt \
			mollit anim id est laborum. Lorem ipsum dolor sit amet, consectetur adipiscing elit, \
			sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim \
			veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo \
			consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum \
			do voluptate velit esse cillum do voluptate velit esse cillum do voluptate velit \
			esse cillum dovoluptate velit esse cillum do";

		assert_noop!(
			DaoFactory::update_dao_metadata(
				RuntimeOrigin::root(),
				0,
				new_metadata.as_bytes().to_vec()
			),
			Error::<Test>::MetadataTooLong
		);
	})
}

#[test]
fn update_dao_metadata_works() {
	new_test_ext().execute_with(|| {
		create_dao();

		let new_metadata =
			"Lorem Lorem dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor \
			incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud \
			exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure \
			dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. \
			Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt \
			mollit anim id est laborum. Lorem ipsum dolor sit amet, consectetur adipiscing elit, \
			sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim \
			veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo \
			consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum do";

		assert_ok!(DaoFactory::update_dao_metadata(
			RuntimeOrigin::root(),
			0,
			new_metadata.as_bytes().to_vec()
		));

		let DaoConfig { metadata, .. } = DaoFactory::daos(0).unwrap().config;
		assert_eq!(metadata.to_vec(), new_metadata.as_bytes().to_vec());
	})
}

#[test]
fn update_dao_policy_invalid_input() {
	new_test_ext().execute_with(|| {
		create_dao();

		let new_policy = json!({
			"invalid_proposal_period": 10,
		});

		assert_noop!(
			DaoFactory::update_dao_policy(
				RuntimeOrigin::root(),
				0,
				serde_json::to_vec(&new_policy).ok().unwrap()
			),
			Error::<Test>::InvalidInput
		);
	})
}

#[test]
fn update_dao_policy_works() {
	new_test_ext().execute_with(|| {
		create_dao();

		let new_proposal_period = 500000;
		let new_policy = json!({
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

		assert_ok!(DaoFactory::update_dao_policy(
			RuntimeOrigin::root(),
			0,
			serde_json::to_vec(&new_policy).ok().unwrap()
		),);

		let DaoPolicy { proposal_period, .. } = DaoFactory::policies(0).unwrap();
		assert_eq!(proposal_period, new_proposal_period);
	})
}

#[test]
fn mint_dao_token_works() {
	new_test_ext().execute_with(|| {
		create_dao();

		assert_ok!(DaoFactory::mint_dao_token(RuntimeOrigin::root(), 0, 100));
	})
}

#[test]
fn spend_dao_funds() {
	new_test_ext().execute_with(|| {
		create_dao();

		assert_ok!(DaoFactory::spend_dao_funds(RuntimeOrigin::root(), 0));
	})
}

#[test]
fn launch_dao_referendum_none_waiting() {
	new_test_ext().execute_with(|| {
		create_dao();

		assert_noop!(
			DaoFactory::launch_dao_referendum(RuntimeOrigin::root(), 0),
			DemocracyError::<Test>::NoneWaiting
		);
	})
}

#[test]
fn bake_dao_referendum() {
	new_test_ext().execute_with(|| {
		create_dao();

		assert_ok!(DaoFactory::bake_dao_referendum(RuntimeOrigin::root(), 0),);
	})
}

#[test]
fn subscribe_dao_already_subscribed() {
	new_test_ext().execute_with(|| {
		create_dao();

		assert_noop!(
			DaoFactory::subscribe(RuntimeOrigin::root(), 0, None, None),
			SubscriptionError::<Test>::AlreadySubscribed
		);
	})
}

#[test]
fn extend_dao_subscription_insufficient_balance() {
	new_test_ext().execute_with(|| {
		create_dao();

		assert_noop!(
			DaoFactory::extend_subscription(RuntimeOrigin::root(), 0),
			BalancesError::<Test>::InsufficientBalance
		);
	})
}

#[test]
fn extend_dao_subscription_works() {
	new_test_ext().execute_with(|| {
		create_dao();

		let dao_account_id = DaoFactory::dao_account_id(0);
		Balances::make_free_balance_be(&dao_account_id, 1_000_000_000_000_000_000);

		assert_ok!(DaoFactory::extend_subscription(RuntimeOrigin::root(), 0));
	})
}

#[test]
fn change_dao_subscription_invalid_tier() {
	new_test_ext().execute_with(|| {
		create_dao();

		assert_noop!(
			DaoFactory::change_subscription(
				RuntimeOrigin::root(),
				0,
				VersionedDaoSubscriptionTier::Default(DaoSubscriptionTierV1::Standard)
			),
			SubscriptionError::<Test>::InvalidSubscriptionTier
		);
	})
}

#[test]
fn schedule_dao_subscription_payment() {
	new_test_ext().execute_with(|| {
		create_dao();

		assert_ok!(DaoFactory::schedule_subscription_payment(RuntimeOrigin::root(), 0));
	})
}

#[test]
fn cancel_dao_subscription_payment() {
	new_test_ext().execute_with(|| {
		create_dao();

		assert_ok!(DaoFactory::schedule_subscription_payment(RuntimeOrigin::root(), 0));

		assert_ok!(DaoFactory::cancel_subscription_payment(RuntimeOrigin::root(), 0));
	})
}

fn create_dao() {
	let account = Public::from_string("/Alice").ok().unwrap();
	Balances::make_free_balance_be(&account, 1_000_000_000_000_000_000);
	let account1 = Public::from_string("/Bob").ok().unwrap();

	let dao_json = get_dao_json();

	assert_ok!(DaoFactory::create_dao(
		RuntimeOrigin::signed(account.clone()),
		vec![account.clone(), account1.clone()],
		vec![account.clone(), account1.clone()],
		serde_json::to_vec(&dao_json).ok().unwrap()
	));
}
