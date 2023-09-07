# Societal Node Testing Guide - Subscription Recurring Payment 

## Launch Rococo Local Testnet

- Please refer to the [Run in Docker](../README.md#run-in-docker) section of the README

## Register Societal Parachain - Para ID 2000

- Connect to Alice validator Relay Chain: `https://cloudflare-ipfs.com/ipns/dotapps.io/?rpc=ws%3A%2F%2Flocalhost%3A9944#/explorer`

- Go to `Developer --> Sudo` section and select `parasSudoWrapper --> sudoScheduleParaInitialize(id, genesis)`

- Use `2000` as parachain `id` and set `parachain` to `true.

- Use [para-2000-genesis-state](../examples/para-2000-genesis-state) file as `genesisHead`

- Use [para-2000-wasm](../examples/para-2000-wasm) file as `validationCode`

- Go to `Network --> Parachains` section and wait for ~2 minutes to check whether the 2000 chain is in the `Parachains` list

- Connect to Societal Parachain(2000): `https://cloudflare-ipfs.com/ipns/dotapps.io/?rpc=ws%3A%2F%2Flocalhost%3A9954#/explorer`

- Go to `Network --> Explorer` and check that blocks are produced and finalized

# Prepare Subscription Tiers

- Open `Developer --> Extrinsics` section

- From the list of available extrinsics select `sudo --> sudo(call)` and then `daoSubscription --> setSubscriptionTier` as callable
  
<img src="images/tiers/Screenshot%202023-07-18%20at%2016.56.47.png" width="600" style="padding-left: 50px;">

- Select `Alice` account to submit the transaction

- Use the following settings for submission

```
tier: Default --> Basic
details: Default
duration: 50 (10 minutes)
price: 100000
fnCallLimit: 100
fnPerBlockLimit: 12
maxMembers: 100
```
- Include and enable all of the options from `palletDetails` section

<img src="images/stablecoin/Screenshot%202023-08-16%20at%2013.53.12.png" width="600" style="padding-left: 50px;">

- `Submit Transaction`

- Go to `Developer --> Chain State` and select `daoSubscription --> subscriptionTiers` 

- Select `Default --> Basic` tier and press `+` button

- Now you can see that you've added a `Basic` subscription tier:

<img src="images/payment/Screenshot%202023-08-25%20at%2012.36.49.png" width="600" style="padding-left: 50px;">

## Create Dao - Societal Node

- Connect to Societal Parachain(2000): `https://cloudflare-ipfs.com/ipns/dotapps.io/?rpc=ws%3A%2F%2Flocalhost%3A9954#/explorer`

- Open `Developer --> Extrinsics` section

- From the list of available extrinsics select `dao --> createDao` callable

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
    "token_id": 1,
    "initial_balance": "100000",
    "metadata": {
      "name": "dao",
      "symbol": "daot",
      "decimals": 2
    }
  },
  "tier": {
    "Default": "Basic"
  }
}
```

- `Submit Transaction`

- Go to `Developer --> Chain state` and select `dao --> daos(u32): Option<DaoPrimitivesDao>` with no options defined

- Check that DAO has been successfully created:

<img src="images/stablecoin/Screenshot 2023-08-16 at 14.07.59.png" width="600" style="padding-left: 50px;">

## Schedule Recurring Payment

### Fund DAO Account

- Go to `Developer --> Chain State`

- Select `dao --> daos` for state query and press `+` button

- Now you can see the basic data of your DAO

<img src="images/polkadotjs/Screenshot%202022-11-18%20at%2012.12.05.png" width="600" style="padding-left: 50px;">

- Copy the `account_id` from the response

- Go to `Accounts --> Accounts` section
  
- Send `1000000` SCTL tokens from `Alice` to DAO Account

<img src="images/payment/Screenshot%202023-08-25%20at%2012.53.06.png" width="600" style="padding-left: 50px;">

### Create Proposal

- Go to `Developer --> Extrinsics` and select `daoCouncil --> propose` 

- Select `Alice` account to submit the transaction

- Set the following parameters for a call

```
daoId: 0
```

- For the proposal call use the following:

```
proposal: dao --> scheduleSubscriptionPayment(daoId)
daoId: 0
lengthBound: 10000
```

<img src="images/payment/Screenshot%202023-08-25%20at%2012.42.53.png" width="600" style="padding-left: 50px;">

- `Submit transaction`

- Vote for the proposal using the guide from [Vote for Council Proposal](./SubscriptionTestingGuide.md/#vote-for-council-proposal)

- Go to `Network --> Explorer` section or `Network --> Scheduler` section and observe that `extendSubscription` event is scheduled

<img src="images/payment/Screenshot%202023-08-25%20at%2012.46.17.png" width="600" style="padding-left: 50px;">

<img src="images/payment/Screenshot%202023-08-25%20at%2012.46.37.png" width="600" style="padding-left: 50px;">

### Subscription Extension

- Stay on the `Network --> Scheduler` page and wait until the timer expires and event is fired

- Go to `Network --> Explorer` section and observe that extrinsic succeeded as well as the new event is scheduled for the next time the subscription should be extended

<img src="images/payment/Screenshot%202023-08-25%20at%2012.59.34.png" width="600" style="padding-left: 50px;">

- Go to `Developer --> Chain State`

- Select `daoSubscription --> subscriptions(u32): Option<DaoPrimitivesDaoSubscription>` for state query and press `+` button

- Observe that subscription is `Active` and has been renewed at the same block the scheduled event has been executed

<img src="images/payment/Screenshot%202023-08-25%20at%2013.01.38.png" width="600" style="padding-left: 50px;">

## Cancel Subscription Payment

- Go to `Developer --> Extrinsics` and select `daoCouncil --> propose` 

- Select `Alice` account to submit the transaction

- Set the following parameters for a call

```
daoId: 0
```

- For the proposal call use the following:

```
proposal: dao --> cancelSubscriptionPayment(daoId)
daoId: 0
lengthBound: 10000
```

<img src="images/payment/Screenshot%202023-08-25%20at%2013.57.37.png" width="600" style="padding-left: 50px;">

- `Submit transaction`

- Vote for the proposal using the guide from [Vote for Council Proposal](./SubscriptionTestingGuide.md/#vote-for-council-proposal)

- Make sure you increase `proposalIndex` while voting for every new council proposal created

- Go to `Network --> Explorer` section and observe that `extendSubscription` event is cancelled

<img src="images/payment/Screenshot%202023-08-25%20at%2014.22.19.png" width="600" style="padding-left: 50px;">

## Confirm subscription expiration

- Go to `Developer --> Chain State`

- Select `daoSubscription --> subscriptions(u32): Option<DaoPrimitivesDaoSubscription>` for state query and press `+` button

- Observe the `until` value for the subscription in `Active` section

<img src="images/payment/Screenshot%202023-08-25%20at%2014.26.19.png" width="600" style="padding-left: 50px;">

- In my case the subscription is `Active` until the block `652`. In your case this value will be different - depending on the extrinsic execution timestamps

- Wait until the `until` block value

- Go to `Developer --> Extrinsics` and select `daoCouncil --> propose` 

- Select `Alice` account to submit the transaction

- Set the following parameters for a call

```
daoId: 0
```

- For the proposal call use the following:

```
proposal: daoCouncilMembers --> addMember(daoId, who)
daoId: 0
lengthBound: 10000
```

<img src="images/payment/Screenshot%202023-08-25%20at%2014.30.19.png" width="600" style="padding-left: 50px;">

- `Submit transaction`

- An error occurs saying that DAO subscription has expired - [Subscription error](../primitives/dao/src/lib.rs#L878):

<img src="images/payment/Screenshot%202023-08-25%20at%2014.31.02.png" width="600" style="padding-left: 50px;">
