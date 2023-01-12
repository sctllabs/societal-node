use crate::{mock::*, BoundedVec, Config, Dao, DaoConfig, DaoPolicy, Error};
use frame_support::{
	assert_noop, assert_ok,
	traits::tokens::fungibles::{metadata::Inspect as MetadataInspect, Inspect},
};
use serde_json::{json, Value};

#[test]
fn create_dao_invalid_input() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			DaoFactory::create_dao(
				Origin::signed(1),
				vec![1, 2],
				r"invalid input".as_bytes().to_vec()
			),
			Error::<Test>::InvalidInput
		);
	})
}

#[test]
fn create_dao_fails_on_string_limits() {
	new_test_ext().execute_with(|| {
		let mut dao_json = get_dao_json();

		dao_json["name"] = Value::String("very long name above the limits".to_string());
		assert_noop!(
			DaoFactory::create_dao(
				Origin::signed(1),
				vec![1, 2],
				serde_json::to_vec(&dao_json).ok().unwrap()
			),
			Error::<Test>::NameTooLong
		);

		dao_json = get_dao_json();
		dao_json["purpose"] = Value::String("very long purpose above the limits".to_string());
		assert_noop!(
			DaoFactory::create_dao(
				Origin::signed(1),
				vec![1, 2],
				serde_json::to_vec(&dao_json).ok().unwrap()
			),
			Error::<Test>::PurposeTooLong
		);

		dao_json = get_dao_json();
		dao_json["metadata"] = Value::String("very long metadata above the limits".to_string());
		assert_noop!(
			DaoFactory::create_dao(
				Origin::signed(1),
				vec![1, 2],
				serde_json::to_vec(&dao_json).ok().unwrap()
			),
			Error::<Test>::MetadataTooLong
		);
	});
}

#[test]
fn create_dao_token_failure() {
	new_test_ext().execute_with(|| {
		let mut dao_json = get_dao_json();

		dao_json["token"] = Value::Null;
		dao_json["token_id"] = Value::Null;
		assert_noop!(
			DaoFactory::create_dao(
				Origin::signed(1),
				vec![1, 2],
				serde_json::to_vec(&dao_json).ok().unwrap()
			),
			Error::<Test>::TokenNotProvided
		);

		dao_json = get_dao_json();
		dao_json["token"] = Value::Null;
		dao_json["token_id"] = json!(1);
		assert_noop!(
			DaoFactory::create_dao(
				Origin::signed(1),
				vec![1, 2],
				serde_json::to_vec(&dao_json).ok().unwrap()
			),
			Error::<Test>::TokenNotExists
		);

		dao_json = get_dao_json();
		dao_json["token"]["token_id"] = json!(2);
		assert_noop!(
			DaoFactory::create_dao(
				Origin::signed(1),
				vec![1, 2],
				serde_json::to_vec(&dao_json).ok().unwrap()
			),
			Error::<Test>::TokenAlreadyExists
		);
	})
}

#[test]
fn create_dao_works() {
	new_test_ext().execute_with(|| {
		let dao = serde_json::to_vec(&get_dao_json()).ok().unwrap();

		assert_ok!(DaoFactory::create_dao(Origin::signed(1), vec![1, 2], dao));

		assert_eq!(DaoFactory::next_dao_id(), 1);

		assert_eq!(
			<Test as Config>::AssetProvider::name(TokenId::get()),
			TokenName::get().as_bytes().to_vec()
		);
		assert_eq!(
			<Test as Config>::AssetProvider::symbol(TokenId::get()),
			TokenSymbol::get().as_bytes().to_vec()
		);
		assert_eq!(<Test as Config>::AssetProvider::decimals(TokenId::get()), TokenDecimals::get());
		assert_eq!(
			<Test as Config>::AssetProvider::minimum_balance(TokenId::get()),
			TokenMinBalance::get().parse::<u128>().unwrap()
		);

		assert_eq!(*Members::get().get(&0).unwrap(), vec![1, 2]);

		assert_eq!(DaoFactory::daos(0).is_some(), true);
		assert_eq!(
			DaoFactory::daos(0).unwrap(),
			Dao {
				founder: 1,
				token_id: 0,
				account_id: 8299986162028932973,
				config: DaoConfig {
					name: BoundedVec::<u8, <Test as Config>::DaoStringLimit>::try_from(
						DaoName::get().as_bytes().to_vec()
					)
					.unwrap(),
					purpose: BoundedVec::<u8, <Test as Config>::DaoStringLimit>::try_from(
						DaoPurpose::get().as_bytes().to_vec()
					)
					.unwrap(),
					metadata: BoundedVec::<u8, <Test as Config>::DaoStringLimit>::try_from(
						DaoMetadata::get().as_bytes().to_vec()
					)
					.unwrap(),
				}
			}
		);

		assert_eq!(DaoFactory::policies(0).is_some(), true);
		assert_eq!(
			DaoFactory::policies(0).unwrap(),
			DaoPolicy {
				proposal_bond: 1,
				proposal_bond_min: 1,
				proposal_bond_max: None,
				proposal_period: 100,
				approve_origin: (1, 2),
			}
		);
	});
}
