
# Societal Node Testing Guide - Subscription Pallet

## Build and Run Node

- Build the node with the following command:

```
cargo b --release
```

- Run node in dev mode so everytime you restart all the data is cleared and it starts from block 0:

```
./target/release/societal-node --dev --enable-offchain-indexing true
```

## Prepare new wallet

- Open Polkadot JS UI

- Connect to `development node --> local node`

<img src="images/polkadotjs/Screenshot%202022-11-18%20at%2015.41.10.png" width="200" style="padding-left: 50px;">

- Open Polkadot wallet extension and press `+` to create a new account

<img src="images/subscription/Screenshot%202023-06-29%20at%2017.26.48.png" width="600" style="padding-left: 50px;">

- Follow the procedure to create a new wallet account

- Go to `Accounts --> Accounts` and check the balance for the account created:

<img src="images/subscription/Screenshot%202023-06-29%20at%2017.33.38.png" width="400" style="padding-left: 50px;">

- Create another account with zero balance repeating the steps above to check subscriptions further on

- Check the balance of `Alice` account that will be used to create a DAO:

<img src="images/subscription/Screenshot%202023-06-29%20at%2017.59.50.png" width="200" style="padding-left: 50px;">

# Basic Subscription Functionality

## Create new DAO

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
    "proposal_period": 3000,
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
  }
}
```

- `Submit Transaction` --> DAO successfully created

### Observe Subscription Information

- Go to `Accounts --> Accounts` and check that balance decreased by `1000 SCTL` tokens(current default basic subscription price):

<img src="images/subscription/Screenshot%202023-06-29%20at%2018.02.23.png" width="600" style="padding-left: 50px;">

- Go to `Developer --> Chain State`
  
- Select `daoSubscription --> subscriptions` and press `+` button

- Now you can see that DAO has subscribed to the basic tier:

<img src="images/subscription/Screenshot%202023-06-29%20at%2018.07.56.png" width="600" style="padding-left: 50px;">

- Observe that `fnBalance` at basic tier is set to 10K function calls - this number will decrease while we continue working with the DAO

## Create Council Proposal

- Go to `Developer --> Extrinsics` and select `daoCouncil --> propose` 

- Use the account that you've created recently(0 SCTL tokens) as a tx submitter
  
- Set the following parameters for a call:

```
daoId: 0
```

- For the proposal call use the following:

```
proposal: daoTreasury --> transferToken
daoId: 0
amount: 1000
beneficiary: select the wallet account you'be recently created
lengthBound: 10000
```

<img src="images/subscription/Screenshot%202023-06-29%20at%2018.20.29.png" width="600" style="padding-left: 50px;">

- `Submit transaction`

- In the submission modal you can observe that `fees of 0` will be applied:

<img src="images/subscription/Screenshot%202023-06-30%20at%2013.40.31.png" width="600" style="padding-left: 50px;">

- Transaction successfully submitted and no gas fee has been charged since the account balance was 0 SCTL

- Please note that DAO calls are also limited by the `fnPerBlockLimit` for now so DAO members can make at most 100 function calls per block as per the current settings

- Go to `Developer --> Chain State`
  
- Select `daoSubscription --> subscriptions` and press `+` button

- You can observe that `fnBalance` has decrease by 1:

<img src="images/subscription/Screenshot%202023-06-30%20at%2013.33.23.png" width="600" style="padding-left: 50px;">

- You can try to create a similar proposal by the 2nd account that you've created earlier receiving the following error:

<img src="images/subscription/Screenshot%202023-07-03%20at%2016.12.05.png" width="400" style="padding-left: 50px;">

- It is custom runtime tx error [Not DAO Member](../primitives/dao/src/lib.rs#L643)

## Vote for Council Proposal

- Go to `Developer --> Chain State`
  
- Select `daoCouncil --> proposals` and press `+` button

- Copy the proposal hash that will be used for voting

- Open `Developer --> Extrinsics` section

- From the list of available extrinsics select `daoCouncil --> vote` callable

- Use the following parameters for the voting call:

```
account: Alice
daoId: 0
proposal: 0x5ed15c6ac27c4bd22a5f084e4be38af2904f2e669abf5b35e7f97975aeefcbf0
index: 0
approve: Yes
```

<img src="images/subscription/Screenshot%202023-07-03%20at%2016.32.45.png" width="600" style="padding-left: 50px;">

- `Submit Transaction`

- repeat the voting step above using the 1st account you've created earlier

- Select `daoCouncil --> close` callable

- Use the following parameters for the voting close call:

```
account: 1st account from the wallet
daoId: 0
proposal: 0x5ed15c6ac27c4bd22a5f084e4be38af2904f2e669abf5b35e7f97975aeefcbf0
index: 0
refTime: 1000000
proofSize: 0
lengthBound: 10000
```

- `Submit Transaction`

- Go to `Developer --> Chain State`
  
- Select `daoCouncil --> proposals` and press `+` button

- Select `daoSubscription --> subscriptions` and press `+` button

- Observe that proposal has been successfully closed. Please make note that `fnBalance` has decreased by 4 overall so we've already made 4 calls for the DAO: `proposal --> voting --> voting --> close`:

<img src="images/subscription/Screenshot%202023-07-03%20at%2016.44.12.png" width="600" style="padding-left: 50px;">

## Subscription extrinsics

All in all here are the [extrinsics](../runtime/src/extensions.rs#L74) that are making use of a DAO subscription so you can try to use them

### DAO Bounties

```
daoBounties --> unassignCurator
daoBounties --> acceptCurator
daoBounties --> awardBounty
daoBounties --> claimBounty
daoBounties --> extendBountyExpiry
```

### DAO Council

```
daoCouncil --> propose
daoCouncil --> proposeWithMeta
daoCouncil --> vote
daoCouncil --> close
```

### DAO Technical Committee

```
daoTechnicalCommittee --> propose
daoTechnicalCommittee --> proposeWithMeta
daoTechnicalCommittee --> vote
daoTechnicalCommittee --> close
```

### DAO Democracy

```
daoDemocracy --> propose
daoDemocracy --> proposeWithMeta
daoDemocracy --> second
daoDemocracy --> vote
daoDemocracy --> delegate
daoDemocracy --> undelegate
daoDemocracy --> unlock
daoDemocracy --> remove_vote
daoDemocracy --> remove_other_vote
```

### DAO Council Members

```
daoCouncilMembers --> changeKey
```

### DAO Technical Committee Members

```
daoTechnicalCommitteeMembers --> changeKey
```

# Limit Subscription Duration

## Update Basic Subscription Tier

- Open `Developer --> Extrinsics` section

- From the list of available extrinsics select `sudo --> sudo(call)` and then `daoSubscription --> setSubscriptionTiers` callable

- Use the following parameters for the call:

```
account: Alice
tiers: Default -> Basic
duration: 5 (subscription will be valid for 30 seconds)
price: 100
fnCallLimit: 100
fnPerBlockLimit: 100
```

<img src="images/subscription/Screenshot%202023-07-03%20at%2017.22.43.png" width="600" style="padding-left: 50px;">

- `Submit Transaction`

- Go to `Developer --> Chain State`
  
- Select `daoSubscription --> subscriptionTiers` and press `+` button

- Observe that changes have been stored

<img src="images/subscription/Screenshot%202023-07-03%20at%2017.25.35.png" width="600" style="padding-left: 50px;">

## Create DAO

- Use the same approach from [Create new Dao](./SubscriptionTestingGuide.md/#create-new-dao) using the following payload:

```json
{
  "name": "dao name 1",
  "purpose": "dao purpose 1",
  "metadata": "dao metadata 1",
  "policy": {
    "proposal_period": 3000,
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
    "token_id": 2,
    "initial_balance": "100000",
    "metadata": {
      "name": "dao",
      "symbol": "daot",
      "decimals": 2
    }
  }
}
```

- Go to `Developer --> Chain State`
  
- Select `daoSubscription --> subscriptions` and press `+` button for `daoId=1`

- Observe that subscription for the DAO expires at some block in the future:

<img src="images/subscription/Screenshot%202023-07-03%20at%2017.47.47.png" width="600" style="padding-left: 50px;">

## Create Proposal

- Wait for ~30 seconds until the DAO subcription expires

- Use the same approach from the [Create Council Proposal](./SubscriptionTestingGuide.md/#create-council-proposal) step using `daoI=1`

- An error occurs saying that subscription has expired - [Subscription Error](../primitives/dao/src/lib.rs#L645):

<img src="images/subscription/Screenshot%202023-07-03%20at%2017.31.39.png" width="400" style="padding-left: 50px;">

# Limit Subscription Function Calls

## Update Basic Subscription Tier

- Open `Developer --> Extrinsics` section

- From the list of available extrinsics select `sudo --> sudo(call)` and then `daoSubscription --> setSubscriptionTiers` callable

- Use the following parameters for the call:

```
account: Alice
tiers: Default -> Basic
duration: 1000000
price: 100
fnCallLimit: 1 (single function call allowed)
fnPerBlockLimit: 100
```

- `Submit Transaction`

## Create DAO

- Use the same approach from [Create new Dao](./SubscriptionTestingGuide.md/#create-new-dao) step using the following payload:

```json
{
  "name": "dao name 2",
  "purpose": "dao purpose 2",
  "metadata": "dao metadata 2",
  "policy": {
    "proposal_period": 3000,
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
    "token_id": 3,
    "initial_balance": "100000",
    "metadata": {
      "name": "dao",
      "symbol": "daot",
      "decimals": 2
    }
  }
}
```

- Go to `Developer --> Chain State`
  
- Select `daoSubscription --> subscriptions` and press `+` button for `daoId=2`

- Observe that subscription for the DAO has a single function call on a balance:

<img src="images/subscription/Screenshot%202023-07-03%20at%2017.53.53.png" width="400" style="padding-left: 50px;">

## Create Proposal

- Use the same approach from the [Create Council Proposal](./SubscriptionTestingGuide.md/#create-council-proposal) step using `2` as `daoId`

- `Submit Transaction`

- Go back to `Developer --> Chain State` selecting `daoSubscription --> subscriptions` and press `+` button for `daoId=2`

- Observe that `fnBalance` is `0` for the DAO

## Proposal Voting

- Open `Developer --> Extrinsics` section

- From the list of available extrinsics select `daoCouncil --> vote` callable

- Use the following parameters for the voting call:

```
account: 1st account from the wallet
daoId: 2
proposal: 0x41e98a57212332578226a52d254f52ffd2b4de23d0226b0c4956d61313cad1f7
index: 0
approve: Yes
```

<img src="images/subscription/Screenshot%202023-07-03%20at%2018.03.17.png" width="600" style="padding-left: 50px;">

- `Submit Transaction`

- An error occurs saying that function limit exceeded - [Subscription Error](../primitives/dao/src/lib.rs#L645):

# Extend Subscription

## Pre-fund DAO Account

- Go to `Developer --> Chain State`
  
- Select `dao --> daos` and press `+` button for `daoId=0`

- Copy `accountId` from the output - `5EYCAe5ijiYfqMFxyJDmHoxzF1VJ4NTsqtRAgdjN3q6pCz51`

<img src="images/subscription/Screenshot%202023-07-04%20at%2010.27.15.png" width="600" style="padding-left: 50px;">

- Send ~5000 SCTL tokens to DAO Account Id via `Accounts` page:

<img src="images/subscription/Screenshot%202023-07-04%20at%2010.29.36.png" width="600" style="padding-left: 50px;">

## Create Proposal

- Open `Developer --> Extrinsics` section

- From the list of available extrinsics select `daoCouncil --> propose` callable

- Use the account that you've created recently(0 SCTL tokens) as a tx submitter
  
- Set the following parameters for a call:

```
daoId: 0
```

- For the proposal call use the following:

```
proposal: dao --> extendSubscription
daoId: 0
lengthBound: 10000
```

<img src="images/subscription/Screenshot%202023-07-04%20at%2010.34.22.png" width="600" style="padding-left: 50px;">

- `Submit transaction`

## Vote for Extension Proposal

- Use the same approach from the [Vote for Council Proposal](./SubscriptionTestingGuide.md/#vote-for-council-proposal) step changing the following params:

```
dao_id: 0
proposalIndex: 1
proposalHash: 0xbca84298f0b0c6823ef4863168bc5c545dc7553560850987b455256dc3dc950e
```

- Make sure you vote for both `Alice` and `1st account from the wallet` to vote the proposal through

- Select `daoCouncil --> close` callable

- Use the following parameters for the voting close call:

```
account: 1st account from the wallet
daoId: 0
proposal: 0xbca84298f0b0c6823ef4863168bc5c545dc7553560850987b455256dc3dc950e
index: 1
refTime: 1000000
proofSize: 0
lengthBound: 10000
```

- `Submit Transaction`

- Go to `Developer --> Chain State`
  
- Select `daoSubscription --> subscriptions` and press `+` button for `daoId=0`

- Observe that DAO subscription has been extended, e.g increased values for `fnBalance` and `until` parameters

<img src="images/subscription/Screenshot%202023-07-04%20at%2010.46.31.png" width="600" style="padding-left: 50px;">
