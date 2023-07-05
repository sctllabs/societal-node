use crate::{mock::*, BoundedVec, Config, Dao, DaoConfig, DaoPolicy, Error};
use dao_primitives::{
	BountyPayoutDelay, BountyUpdatePeriod, DaoPolicyProportion, DaoStatus, DaoToken,
	TreasurySpendPeriod,
};
use frame_support::{
	assert_noop, assert_ok,
	traits::tokens::fungibles::{metadata::Inspect as MetadataInspect, Inspect},
};
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
fn create_dao_works() {
	new_test_ext().execute_with(|| {
		let account = Public::from_string("/Alice").ok().unwrap();
		Balances::make_free_balance_be(&account, 1_000_000_000_000_000_000);
		let account1 = Public::from_string("/Bob").ok().unwrap();

		let dao = serde_json::to_vec(&get_dao_json()).ok().unwrap();

		assert_ok!(DaoFactory::create_dao(
			RuntimeOrigin::signed(account.clone()),
			vec![account.clone(), account1.clone()],
			vec![],
			dao
		));

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
				governance: None,
				bounty_payout_delay: BountyPayoutDelay(14400),
				bounty_update_period: BountyUpdatePeriod(14400),
				spend_period: TreasurySpendPeriod(14400),
			}
		);
	});
}
