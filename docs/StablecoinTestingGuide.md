# Societal Node Subscription Stablecoin Payment Testing Guide

## Launch Rococo Local Testnet

- Please refer to the [Run Local Rococo Testnet ](../README.md#run-rococo-local-testnet) section of the README

## Register Asset Hub Parachain - Para ID 1000

- Connect to Alice validator Relay Chain: `https://cloudflare-ipfs.com/ipns/dotapps.io/?rpc=ws%3A%2F%2Flocalhost%3A9944#/explorer`

- Go to `Developer --> Sudo` section and select `registrar --> forceRegister(who, deposit, id, genesisHead, validationCode)`

- Use the following parameters for the call:

```
who: Alice
deposit: 0
id: 1000
```

- Use [para-1000-genesis-state](../examples/para-1000-genesis-state) file as `genesisHead`

- Use [para-1000-wasm](../examples/para-1000-wasm) file as `validationCode`

- Go to `Network --> Parachains` section and wait for ~2 minutes to check whether the 1000 chain is in the `Parathreads` list

- Go to `Developer --> Sudo` section and select `assignedSlots --> assignPermParachainSlot(id)`

- Use `1000` as the parachain `id` and `Submit Sudo`

- Go to `Network --> Parachains` section and wait for ~2 minutes to check whether the 1000 chain is in the `Parachains` list

- Connect to the Asset Hub(1000): `https://cloudflare-ipfs.com/ipns/dotapps.io/?rpc=ws%3A%2F%2Flocalhost%3A9956#/explorer`

- Go to `Network --> Explorer` and check that blocks are produced and finalized

## Register Societal Parachain - Para ID 2000

- Connect to Alice validator Relay Chain: `https://cloudflare-ipfs.com/ipns/dotapps.io/?rpc=ws%3A%2F%2Flocalhost%3A9944#/explorer`

- Go to `Developer --> Sudo` section and select `parasSudoWrapper --> sudoScheduleParaInitialize(id, genesis)`

- Use `2000` as parachain `id` and set `parachain` to `true.

- Use [para-2000-genesis-state](../examples/para-2000-genesis-state) file as `genesisHead`

- Use [para-2000-wasm](../examples/para-2000-wasm) file as `validationCode`

- Go to `Network --> Parachains` section and wait for ~2 minutes to check whether the 2000 chain is in the `Parachains` list

- Connect to Societal Parachain(2000): `https://cloudflare-ipfs.com/ipns/dotapps.io/?rpc=ws%3A%2F%2Flocalhost%3A9954#/explorer`

- Go to `Network --> Explorer` and check that blocks are produced and finalized

## Open HRMP Channel from Asset Hub(1000) to Societal Node(2000)

- Refer to the [Open message passing channels](https://docs.substrate.io/tutorials/build-a-parachain/open-message-passing-channels/) section of Polkadot documentation

- Use the following sovereign accounts for Parachains connected:

```
ParaId 1000 - 5Ec4AhPZk8STuex8Wsi9TwDtJQxKqzPJRCH7348Xtcs9vZLJ
ParaId 2000 - 5Ec4AhPUwPeyTFyuhGuBbD224mY85LKLMSqSSo33JYWCazU4
```

- Please make sure you open bi-directional HRMP channel completing the guide from the previous step for both 2000 and 2001 parachains.

- Check whether the HRMP channels are opened in both directions:

<img src="images/stablecoin/Screenshot 2023-08-16 at 11.56.24.png" width="600" style="padding-left: 50px;">

## Create Asset - Asset Hub

### Create Asset

- Connect to the Asset Hub(1000): `https://cloudflare-ipfs.com/ipns/dotapps.io/?rpc=ws%3A%2F%2Flocalhost%3A9956#/explorer`

- Go to `Developer --> Sudo` section and select `asset --> forceCreate(id, owner, isSufficient, minBalance)`

- Use the following parameters for the call:

```
id: 1984 - similar to `Tether USD` on Polkadot Asset Hub
owner: Alice
isSufficient: true
minBalance: 1
```

<img src="images/stablecoin/Screenshot 2023-08-16 at 12.06.58.png" width="600" style="padding-left: 50px;">

- `Submit Transaction`

### Mint Tokens

- Go to `Developer --> Extrinsics` and select `asset --> mint(id, beneficiary, amount)` as the extrinsic to mint some USDTs

- Use the following parameters for the call:

```
id: 1984 - similar to `Tether USD` on Polkadot Asset Hub
beneficiary: Alice
amount: 1000000000
```

<img src="images/stablecoin/Screenshot 2023-08-16 at 12.11.51.png" width="600" style="padding-left: 50px;">

- `Submit Transaction`

- Go to `Network --> Assets` and check the balance of the USDT token:

<img src="images/stablecoin/Screenshot 2023-08-16 at 12.15.07.png" width="600" style="padding-left: 50px;">

## Create Asset Location Mapping - Societal Node 

### Create Asset

- Connect to Societal Parachain(2000): `https://cloudflare-ipfs.com/ipns/dotapps.io/?rpc=ws%3A%2F%2Flocalhost%3A9954#/explorer`

- Go to `Developer --> Extrinsics` section and select `sudo --> sudo(call)`

- Select `asset --> forceCreate(id, owner, isSufficient, minBalance)` as the call function

- Use the following parameters for the call:

<img src="images/stablecoin/Screenshot 2023-08-16 at 12.30.34.png" width="600" style="padding-left: 50px;">

- `Submit Transaction`

- Go to `Network --> Assets` and check that token has been created:

<img src="images/stablecoin/Screenshot 2023-08-16 at 12.30.53.png" width="600" style="padding-left: 50px;">

### Create Asset Location Mapping

- Go to `Developer --> Sudo` section and select `xcAssetRegistry --> registerAssetLocation(assetLocation, assetId)`

- Use the following parameters for the call:

<img src="images/stablecoin/Screenshot 2023-08-16 at 12.21.54.png" width="600" style="padding-left: 50px;">

- `Submit Transaction`

- Go to `Developer --> Chain state` and select `xcAssetRegistry --> assetIdToLocation(u128): Option<XcmVersionedMultiLocation>` with no options defined

- Check that the asset id location mapping has been created:

<img src="images/stablecoin/Screenshot 2023-08-16 at 12.25.11.png" width="600" style="padding-left: 50px;">

### Create Asset Units per Second Mapping

- Go to `Developer --> Sudo` section and select `xcAssetRegistry --> setAssetUnitsPerSecond(assetLocation, unitsPerSecond)`

- Use the following parameters for the call:

<img src="images/stablecoin/Screenshot 2023-08-16 at 12.52.56.png" width="600" style="padding-left: 50px;">

- `Submit Transaction`

- Go to `Developer --> Chain state` and select `xcAssetRegistry --> assetLocationUnitsPerSecond(XcmVersionedMultiLocation): Option<u128>` with no options defined

- Check that the asset id location mapping has been created:

<img src="images/stablecoin/Screenshot 2023-08-16 at 12.54.02.png" width="600" style="padding-left: 50px;">

## Send tokens from Asset Hub to Societal Node

- Connect to the Asset Hub(1000): `https://cloudflare-ipfs.com/ipns/dotapps.io/?rpc=ws%3A%2F%2Flocalhost%3A9956#/explorer`

- Go to `Developer --> Extrinsics` section and select `polkadotXcm --> limitedReserveTransferAssets(dest, beneficiary, assets, feeAssetItem, weightLimit)`

- Use the following parameters for the call:

<img src="images/stablecoin/Screenshot 2023-08-16 at 12.48.46.png" width="600" style="padding-left: 50px;">
<img src="images/stablecoin/Screenshot 2023-08-16 at 12.48.53.png" width="600" style="padding-left: 50px;">
<img src="images/stablecoin/Screenshot 2023-08-16 at 12.49.02.png" width="600" style="padding-left: 50px;">

- `Submit Transaction`

- Go to `Network --> Explorer` and check that `xcm` message has been sent:

<img src="images/stablecoin/Screenshot 2023-08-16 at 12.57.21.png" width="600" style="padding-left: 50px;">

- Connect to Societal Parachain(2000): `https://cloudflare-ipfs.com/ipns/dotapps.io/?rpc=ws%3A%2F%2Flocalhost%3A9954#/explorer`

- Go to `Network --> Explorer` and check `xcm` logs:

<img src="images/stablecoin/Screenshot 2023-08-16 at 12.58.50.png" width="600" style="padding-left: 50px;">

- Go to `Network --> Assets` and check supply for the asset:

<img src="images/stablecoin/Screenshot 2023-08-16 at 13.00.02.png" width="600" style="padding-left: 50px;">

## Create Subscription Tier - Societal Node

- Connect to Societal Parachain(2000): `https://cloudflare-ipfs.com/ipns/dotapps.io/?rpc=ws%3A%2F%2Flocalhost%3A9954#/explorer`

- Open `Developer --> Extrinsics` section

- From the list of available extrinsics select `sudo --> sudo(call)` and then `daoSubscription --> setSubscriptionTiers` callable

- Use the following parameters for the call:

<img src="images/stablecoin/Screenshot 2023-08-16 at 13.49.52.png" width="600" style="padding-left: 50px;">

- Enable all of the DAO pallet functions down below:

<img src="images/stablecoin/Screenshot 2023-08-16 at 13.53.12.png" width="600" style="padding-left: 50px;">

- `Submit Transaction`

## Create Dao - Societal Node

- Connect to Societal Parachain(2000): `https://cloudflare-ipfs.com/ipns/dotapps.io/?rpc=ws%3A%2F%2Flocalhost%3A9954#/explorer`

- Open `Developer --> Extrinsics` section

- Use the following parameters for the call:

<img src="images/stablecoin/Screenshot 2023-08-16 at 13.57.29.png" width="600" style="padding-left: 50px;">

- Use the following `json` as the payload for the `DAO`:

```
{
  "name": "dao name 1",
  "purpose": "dao purpose",
  "metadata": "dao metadata",
  "policy": {
    "proposal_period": 300,
    "spend_period": 130,
    "governance": {
      "GovernanceV1": {
        "enactment_period": 20,
        "launch_period": 200,
        "voting_period": 200,
        "vote_locking_period": 20,
        "fast_track_voting_period": 300000,
        "cooloff_period": 30,
        "minimum_deposit": 1
      }
    }
  },
  "token": {
    "token_id": 2,
    "initial_balance": "100000",
    "metadata": {
      "name": "dao",
      "symbol": "daot",
      "decimals": 2
    }
  },
  "tier": {
    "Default": "Basic"
  },
  "subscription_token": 1984000
}
```

- `Submit Transaction`

- Go to `Developer --> Chain state` and select `dao --> daos(u32): Option<DaoPrimitivesDao>` with no options defined

- Check that DAO has been successfully created:

<img src="images/stablecoin/Screenshot 2023-08-16 at 14.07.59.png" width="600" style="padding-left: 50px;">

- Go to `Developer --> Chain state` and select `assets --> account(u128, AccountId32): Option<PalletDaoAssetsAssetAccount>` with `1984000` as the `assetId`

- Check that some asset balance has been transferred to the chain treasury account:

<img src="images/stablecoin/Screenshot 2023-08-16 at 14.09.45.png" width="600" style="padding-left: 50px;">

- So here is why the balance value for the treasury account id is `2840`. In this particular case `Alice` has created 2 DAOs(wasn't included in this guide) - that's why Alice was charged with 2000 USDt tokens. `840` tokens have been charged while sending tokens from the `AssetHub` to `Societal Node` covering the `xcm` execution.

- For more info about `DAO subscription` and `tiers`, please refer to the [SubscriptionTestingGuide](./SubscriptionTestingGuide.md) and [TiersTestingGuide](./TiersTestingGuide.md)
