use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

#[derive(Copy, Clone, Eq, PartialEq, Default, Debug, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub struct Vote<Balance> {
	pub aye: bool,
	pub balance: Balance,
}

#[derive(Encode, MaxEncodedLen, Decode, Copy, Clone, Eq, PartialEq, TypeInfo)]
pub struct AccountVote<AccountId, Balance> {
	pub who: AccountId,
	pub vote: Vote<Balance>,
}
