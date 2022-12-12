use frame_support::sp_runtime::traits::Hash;
use sp_core::H256;

pub fn hash<Runtime>(data: &[u8]) -> H256
where
	Runtime: frame_system::Config,
	H256: From<<Runtime as frame_system::Config>::Hash>,
{
	<Runtime as frame_system::Config>::Hashing::hash(data).into()
}
