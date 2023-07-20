
# Societal Node Testing Guide - Subscription Tiers

## Build and Run Node

- Build the node with the following command

```
cargo b --release
```

- Run node in dev mode so everytime you restart all the data is cleared and it starts from block 0:

```
./target/release/societal-node --dev --enable-offchain-indexing true
```

## Prepare new wallet

- Use preparations steps from [Prepare new wallet](./SubscriptionTestingGuide.md/#prepare-new-wallet) in [Subscription Testing Guide](./SubscriptionTestingGuide.md)

# Prepare Subscription Tiers

- Open `Developer --> Extrinsics` section

- From the list of available extrinsics select `sudo --> sudo(call)` and then `daoSubscription --> setSubscriptionTier` as callable
  
<img src="images/tiers/Screenshot%202023-07-18%20at%2016.56.47.png" width="600" style="padding-left: 50px;">

- Select `Alice` account to submit the transaction

- Use the following settings for submission

```
tier: Default --> Basic
details: Default
duration: 432000 (one month)
price: 1000000000000000
fnCallLimit: 100
fnPerBlockLimit: 12
maxMembers: 3
```

- Include the following options

`dao`
```
updateDaoMetadata: yes
updateDaoPolicy: yes
mintDaoToken: no
```

`council` - `yes` to all options

`councilMembership` - `yes` to all options

`treasury` - `yes` to all options

<img src="images/tiers/Screenshot%202023-07-18%20at%2017.20.23.png" width="600" style="padding-left: 50px;">

- `Submit transaction`

- Go to `Developer --> Chain State` and select `daoSubscription --> subscriptionTiers` 

- Select `Default --> Basic` tier and press `+` button

- Now you can see that you've added a `Basic` subscription tier:

<img src="images/tiers/Screenshot%202023-07-18%20at%2017.21.04.png" width="600" style="padding-left: 50px;">

- Go back to `Developer --> Extrinsics` section

- From the list of available extrinsics select `sudo --> sudo(call)` and then `daoSubscription --> setSubscriptionTier` as callable

- Select `Alice` account to submit the transaction

- Use the following settings for submission

```
tier: Default --> Standard
details: Default
duration: 432000 (one month)
price: 2000000000000000
fnCallLimit: 1000
fnPerBlockLimit: 12
maxMembers: 100
```

- Include the following options

`dao` - `yes` to all options

`bounties` - `yes` to all options

`council` - `yes` to all options

`councilMembership` - `yes` to all options

`democracy` - `yes` to all options

`treasury` - `yes` to all options

- `Submit transaction`

- Go to `Developer --> Chain State` and select `daoSubscription --> subscriptionTiers` 

- Select `Default --> Standard` tier and press `+` button

- Now you can see that you've added a `Standard` subscription tier:

<img src="images/tiers/Screenshot%202023-07-18%20at%2017.23.32.png" width="600" style="padding-left: 50px;">

# Create new DAO

- Open `Developer --> Extrinsics` section

- From the list of available extrinsics select `dao --> createDao` callable

<img src="images/subscription/Screenshot%202023-06-29%20at%2018.03.56.png" width="600" style="padding-left: 50px;">

- Select `Alice` account to submit the transaction

- Select the 1st account that you've just created to make sure it is added as a council to the DAO 

- Select account you want to be a part of the technical committee of the DAO - you can leave it empty for now since the tech committee group is not yet utilized in the organization

- Use the following json as an example of a DAO data payload:

```json
{
  "name": "dao name",
  "purpose": "dao purpose",
  "metadata": "dao metadata",
  "policy": {
    "proposal_period": 50,
    "governance": {
      "GovernanceV1": {
        "enactment_period": 30,
        "launch_period": 30,
        "voting_period": 30,
        "vote_locking_period": 30,
        "fast_track_voting_period": 30,
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

- `Submit Transaction` --> DAO successfully created

## Observe Subscription Information

- Go to `Accounts --> Accounts` and check that balance decreased by `1000 SCTL` tokens(current default basic subscription price)

<img src="images/subscription/Screenshot%202023-06-29%20at%2018.02.23.png" width="600" style="padding-left: 50px;">

- Go to `Developer --> Chain State`
  
- Select `daoSubscription --> subscriptions` and press `+` button using `0` as `daoId`

- Now you can see that DAO has subscribed to the basic tier:

<img src="images/tiers/Screenshot%202023-07-18%20at%2017.36.49.png" width="600" style="padding-left: 50px;">

# Subscription Tier based Pallet/Functions

## Create Democracy Proposal - Direct call - Pallet disabled

- Let's check the direct call to the pallet disabled. In our case - `daoDemocracy`

- Go to `Developer --> Extrinsics` and select `daoDemocracy --> propose` 

- Use the account that you've created first(0 SCTL tokens) as a tx submitter
  
- Set the following parameters for a call

```
daoId: 0
```

- For the proposal call use the following

```
proposal: Inline
inline: 0x30000000000090b5ab205c6974c9ea841be688864633dc9ca8a357843eeacf2314649965fe22
value: 100
```

<img src="images/tiers/Screenshot%202023-07-18%20at%2017.52.17.png" width="600" style="padding-left: 50px;">

- `Submit transaction`

- An error occurs saying that function is disabled - [Function call disabled](../primitives/dao/src/lib.rs#L779):

<img src="images/tiers/Screenshot%202023-07-19%20at%2013.19.20.png" width="300" style="padding-left: 50px;">

## Create Council Proposal - Proposal call - Pallet is disabled

- Let's try to create proposal with the call to the disabled pallet inside

- Go to `Developer --> Extrinsics` and select `daoCouncil --> propose` 

- Use the account that you've created first(0 SCTL tokens) as a tx submitter
  
- Set the following parameters for a call

```
daoId: 0
```

- For the proposal call use the following

```
proposal: daoBounties --> createBounty
daoId: 0
amount: 100
description: some description
lengthBound: 10000
```

<img src="images/tiers/Screenshot%202023-07-18%20at%2017.40.39.png" width="600" style="padding-left: 50px;">

- `Submit transaction`

- An error occurs saying that function is disabled - [Function call disabled](../primitives/dao/src/lib.rs#L779):

<img src="images/tiers/Screenshot%202023-07-19%20at%2013.19.20.png" width="300" style="padding-left: 50px;">

## Create Council Proposal - Proposal call - Pallet function is disabled

- Let's try to create proposal with the call to the enabled pallet but disabled function inside

- Go to `Developer --> Extrinsics` and select `daoCouncil --> propose` 

- Use the account that you've created first(0 SCTL tokens) as a tx submitter
  
- Set the following parameters for a call

```
daoId: 0
```

- For the proposal call use the following

```
proposal: dao --> mintDaoToken
daoId: 0
amount: 100
lengthBound: 10000
```

<img src="images/tiers/Screenshot%202023-07-18%20at%2017.47.46.png" width="600" style="padding-left: 50px;">

- `Submit transaction`

- An error occurs saying that function is disabled - [Function call disabled](../primitives/dao/src/lib.rs#L779):

<img src="images/tiers/Screenshot%202023-07-19%20at%2013.19.20.png" width="300" style="padding-left: 50px;">

## Create Council Proposal - Proposal call - Success

- Go to `Developer --> Extrinsics` and select `daoCouncil --> propose` 

- Use the account that you've created first(0 SCTL tokens) as a tx submitter
  
- Set the following parameters for a call

```
daoId: 0
```

- For the proposal call use the following:

```
proposal: daoTreasury --> transferToken
daoId: 0
amount: 1000
beneficiary: select the wallet account you've created first
lengthBound: 10000
```

<img src="images/subscription/Screenshot%202023-06-29%20at%2018.20.29.png" width="600" style="padding-left: 50px;">

- `Submit transaction`

- Transaction successfully submitted and no gas fee has been charged since the account balance was 0 SCTL

- Vote for the proposal using the guide from [Vote for Council Proposal](./SubscriptionTestingGuide.md/#vote-for-council-proposal)

# DAO Max Members

- Let's check subscription tier limit on max members(in fact - DAO token holders) allowed

- Go to `Developer --> Chain State`
  
- Select `assets --> asset` and press `+` button

- Observe that DAO token has 2 sufficient accounts: the 1st one is the DAO itself and the 2nd one is for the account you've sent tokens to via proposal in the previous step:

<img src="images/tiers/Screenshot%202023-07-19%20at%2013.21.32.png" width="600" style="padding-left: 50px;">

- Repeat steps from [Create Council Proposal - Proposal call - Success](./TiersTestingGuide.md/#create-council-proposal---proposal-call---success) 2 times more using different accounts as a `beneficiary`, e.g. `Bob` and `Charlie`. Don't forget to increase the `proposal index` while voting and closing the proposal.

- Go to `Developer --> Chain State`
  
- Select `assets --> asset` and press `+` button for `1` as an asset id

- Observe that the amount of token holders is 4

<img src="images/tiers/Screenshot%202023-07-19%20at%2013.32.32.png" width="600" style="padding-left: 50px;">

- Let's try to create another proposal similar to the one created in [Create Council Proposal - Proposal call - Success](./TiersTestingGuide.md/#create-council-proposal---proposal-call---success)

- An error occurs - [Too Many Members](../primitives/dao/src/lib.rs#L781)

<img src="images/tiers/Screenshot%202023-07-19%20at%2013.30.42.png" width="300" style="padding-left: 50px;">

- Now the DAO functions are locked until the amount of DAO members goes below the `max members` limit

## Unlock DAO

- Go to `Network --> Assets` section and select `Balances`

<img src="images/tiers/Screenshot%202023-07-19%20at%2013.46.06.png" width="600" style="padding-left: 50px;">

- Click on `Send` button for `Bob` account to send all of his tokens to `Charlie`

<img src="images/tiers/Screenshot%202023-07-19%20at%2014.20.10.png" width="600" style="padding-left: 50px;">

- Set `10` as the amount and uncheck `keep-alive` check to make sure the account gets rid of all of his tokens

- `Send` and `Sign and Submit` transaction

- Observe that `Bob` account has now been removed from the list of token holders

<img src="images/tiers/Screenshot%202023-07-19%20at%2014.31.20.png" width="600" style="padding-left: 50px;">

- You can also check asset state on `Developer --> Chain State` observing the `accounts` number decreased to `3`

<img src="images/tiers/Screenshot%202023-07-19%20at%2014.32.21.png" width="600" style="padding-left: 50px;">

- Now you can try to create a new proposal similar to the one created in [Create Council Proposal - Proposal call - Success](./TiersTestingGuide.md/#create-council-proposal---proposal-call---success).

- Observe that transaction succeeded and proposal has been created

# Change Subscription Tier

## Pre-fund DAO Account

- Go to `Developer --> Chain State`
  
- Select `dao --> daos` and press `+` button for `daoId=0`

- Copy `accountId` from the output - `5EYCAe5ijiYfqMFxyJDmHoxzF1VJ4NTsqtRAgdjN3q6pCz51`

<img src="images/subscription/Screenshot%202023-07-04%20at%2010.27.15.png" width="600" style="padding-left: 50px;">

- Send ~5000 SCTL tokens to DAO Account Id via `Accounts` page:

<img src="images/subscription/Screenshot%202023-07-04%20at%2010.29.36.png" width="600" style="padding-left: 50px;">

## Change Subscription Tier - Council Proposal

- Go to `Developer --> Extrinsics` and select `daoCouncil --> propose` 

- Use the account that you've created first(0 SCTL tokens) as a tx submitter
  
- Set the following parameters for a call

```
daoId: 0
```

- For the proposal call use the following:

```
proposal: dao --> changeSubscription
daoId: 0
tier: Default --> Standard
lengthBound: 10000
```

<img src="images/tiers/Screenshot%202023-07-19%20at%2015.02.27.png" width="600" style="padding-left: 50px;">

- `Submit transaction`

- Vote for the proposal the same way it described in [Create Council Proposal - Proposal call - Success](./TiersTestingGuide.md/#create-council-proposal---proposal-call---success). Use `4` as the `proposal index` 

- After voting and closing the proposal go to `Developer --> Chain State`

- Select `daoSubscription --> subscriptions` and press `+` button with `0` as `daoId`

- Observe that DAO has changed subcription tier to `Standard`:

<img src="images/tiers/Screenshot%202023-07-19%20at%2015.18.27.png" width="600" style="padding-left: 50px;">

- Now you can play with the functionality described above(creating proposals or using direct function calls) taking into account that you've enabled `bounties` and `daoDemocracy` pallets and its functions
