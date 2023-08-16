use super::*;
use frame_support::storage_alias;
use log::info;

const LOG_TARGET: &str = "dao-subscription";

pub type SubscriptionV1<T> = DaoSubscriptionV1<
	<T as frame_system::Config>::BlockNumber,
	VersionedDaoSubscriptionTier,
	VersionedDaoSubscriptionDetails<<T as frame_system::Config>::BlockNumber, BalanceOf<T>>,
>;

type SubscriptionDetailsOf<T> = DaoSubscriptionDetails<
	<T as frame_system::Config>::BlockNumber,
	BalanceOf<T>,
	TokenBalances<AssetIdOf<T>, BalanceOf<T>, TokenBalancesLimitOf<T>>,
>;

pub mod v4 {
	use super::*;

	#[storage_alias]
	pub(super) type Subscriptions<T: Config> =
		StorageMap<Pallet<T>, Blake2_128Concat, DaoId, SubscriptionV1<T>, OptionQuery>;

	#[storage_alias]
	pub(super) type SubscriptionTiers<T: Config> = StorageMap<
		Pallet<T>,
		Blake2_128Concat,
		VersionedDaoSubscriptionTier,
		VersionedDaoSubscriptionDetails<<T as frame_system::Config>::BlockNumber, BalanceOf<T>>,
		OptionQuery,
	>;
}

pub fn migrate_to_v5<T: Config>() -> Weight {
	let onchain_version = Pallet::<T>::on_chain_storage_version();
	// migrate to v5
	if onchain_version < 5 {
		SubscriptionTiers::<T>::translate(
			|_: VersionedDaoSubscriptionTier,
			 details: VersionedDaoSubscriptionDetails<
				<T as frame_system::Config>::BlockNumber,
				BalanceOf<T>,
			>| { Some(convert_subscription_details_from_v4_to_v5::<T>(details)) },
		);

		info!(target: LOG_TARGET, " <<< DaoSubscriptionTiers storage updated!");

		Subscriptions::<T>::translate(|k: DaoId, subscription: SubscriptionV1<T>| {
			info!(target: LOG_TARGET, "     Migrated subscription for {:?}...", k);

			let DaoSubscriptionV1 {
				tier,
				details,
				subscribed_at,
				last_renewed_at,
				status,
				fn_balance,
				fn_per_block,
				..
			} = subscription;

			Some(DaoSubscription {
				tier,
				details: convert_subscription_details_from_v4_to_v5::<T>(details),
				token_id: None,
				subscribed_at,
				last_renewed_at,
				status,
				fn_balance,
				fn_per_block,
			})
		});

		// Update storage version.
		StorageVersion::new(5).put::<Pallet<T>>();
		// Very inefficient, mostly here for illustration purposes.
		let count = Subscriptions::<T>::iter().count();
		info!(
			target: LOG_TARGET,
			" <<< DaoSubscription storage updated! Migrated {} subscriptions ✅", count
		);
		// Return the weight consumed by the migration.
		T::DbWeight::get().reads_writes(count as u64 + 1, count as u64 + 1)
	} else {
		info!(target: LOG_TARGET, " >>> Unused migration!");
		// We don't do anything here.
		Weight::zero()
	}
}

fn convert_subscription_details_from_v4_to_v5<T: pallet::Config>(
	details: VersionedDaoSubscriptionDetails<
		<T as frame_system::Config>::BlockNumber,
		BalanceOf<T>,
	>,
) -> SubscriptionDetailsOf<T> {
	match details {
		VersionedDaoSubscriptionDetails::Default(DaoSubscriptionDetailsV1 {
			duration,
			price,
			fn_call_limit,
			fn_per_block_limit,
			max_members,
			dao,
			bounties,
			council,
			tech_committee,
			democracy,
			council_membership,
			tech_committee_membership,
			treasury,
			..
		}) => DaoSubscriptionDetails {
			duration,
			price,
			token_prices:
				BoundedVec::<(AssetIdOf<T>, BalanceOf<T>), TokenBalancesLimitOf<T>>::default(),
			fn_call_limit,
			fn_per_block_limit,
			max_members,
			pallet_details: DaoPalletSubscriptionDetails {
				dao,
				bounties,
				council,
				tech_committee,
				democracy,
				council_membership,
				tech_committee_membership,
				treasury,
			},
		},
	}
}
