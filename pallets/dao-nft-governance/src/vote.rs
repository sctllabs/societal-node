use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

#[derive(Copy, Clone, Eq, PartialEq, Default, Debug, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub struct Vote<Item> {
	pub aye: bool,
	pub item: Item,
}

#[derive(Encode, MaxEncodedLen, Decode, Copy, Clone, Eq, PartialEq, TypeInfo)]
pub struct AccountVote<AccountId, Item> {
	pub who: AccountId,
	pub vote: Vote<Item>,
}
