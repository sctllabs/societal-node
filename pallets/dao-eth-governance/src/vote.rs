use codec::{Decode, Encode, EncodeLike, Input, MaxEncodedLen, Output};
use scale_info::TypeInfo;

#[derive(Copy, Clone, Eq, PartialEq, Default, Encode, Decode, MaxEncodedLen, TypeInfo)]
pub struct Vote<Balance> {
	pub aye: bool,
	balance: Balance,
}

// impl Encode for Vote {
// 	fn encode_to<T: Output + ?Sized>(&self, output: &mut T) {
// 		output.push_byte(u8::from(self.conviction) | if self.aye { 0b1000_0000 } else { 0 });
// 	}
// }
//
// impl MaxEncodedLen for Vote {
// 	fn max_encoded_len() -> usize {
// 		1
// 	}
// }
//
// impl EncodeLike for Vote {}
//
// impl Decode for Vote {
// 	fn decode<I: Input>(input: &mut I) -> Result<Self, codec::Error> {
// 		let b = input.read_byte()?;
// 		Ok(Vote {
// 			aye: (b & 0b1000_0000) == 0b1000_0000,
// 		})
// 	}
// }

// impl TypeInfo for Vote {
// 	type Identity = Self;
//
// 	fn type_info() -> scale_info::Type {
// 		scale_info::Type::builder()
// 			.path(scale_info::Path::new("Vote", module_path!()))
// 			.composite(
// 				scale_info::build::Fields::unnamed()
// 					.field(|f| f.ty::<u8>().docs(&["Raw vote byte, encodes aye + conviction"])),
// 			)
// 	}
// }
