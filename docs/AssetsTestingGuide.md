
# Assets Custom Pallet Testing Guide

## Create new DAO

- Open Polkadot JS UI

- Connect to `development node --> local node`

<img src="images/polkadotjs/Screenshot%202022-11-18%20at%2015.41.10.png" width="200" style="padding-left: 50px;">

- Open `Developer --> Extrinsics` section

- From the list of available extrinsics select `dao --> createDao` callable

<img src="images/assets/Screenshot%202023-02-06%20at%2015.05.35.png" width="600" style="padding-left: 50px;">

- Select accounts you want to be a part of the council of the DAO

- Select accounts you want to be a part of the technical committee of the DAO

- Use the following json as an example of a DAO data payload:

```json
{
  "name": "dao name",
  "purpose": "dao purpose",
  "metadata": "dao metadata",
  "policy": {
    "proposal_period": 30,
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

### Observe Asset Information

- Go to `Developer --> Chain State`

- Select `assets --> asset` from query list

- Use `token_id=1` since we used it while creating the DAO and press `+` button

- You can observe that a new token owned by the DAO was created

<img src="images/assets/Screenshot%202023-02-06%20at%2015.07.55.png" width="300" style="padding-left: 50px;">

## Distribute DAO Governance Token

### Proposal Submission

Let's create proposals to distribute DAO governance token:

- Go to `Developer --> Extrinsics` and select `daoCouncil --> propose`

<img src="images/assets/Screenshot%202023-02-06%20at%2015.14.03.png" width="600" style="padding-left: 50px;">

- Use the following parameters for the council proposal:

```
daoId: 0
```

- For the proposal call use the following:

```
proposal: dao --> transferToken
daoId: 0
amount: 45000
lengthBound: 10000
```

- `Submit Transaction`

- Go to `Developer --> Chain State`

- Select `daoCouncil --> proposals` and press `+` button

<img src="images/polkadotjs/Screenshot%202022-11-18%20at%2012.41.09.png" width="600" style="padding-left: 50px;">

- Copy the proposal hash from the response payload

### Proposal Voting

- Go to `Developer --> Extrinsics` and select `daoCouncil --> vote`

<img src="images/polkadotjs/Screenshot%202022-11-18%20at%2012.42.04.png" width="600" style="padding-left: 50px;">

- Submit Transaction

- Since we're using `AtLeast [1, 2]` as a default policy for council proposal, we can close voting

- To close the proposal prematurely before the end time select `daoCouncil --> close` with the following parameters

```
daoId: 0
proposalHash: <proposal hash from the steps above>
proposalIndex: 0
refTime: 1000000000
proofSize: 0
lengthBound: 10000
```

<img src="images/assets/Screenshot%202023-02-09%20at%2013.44.07.png" width="600" style="padding-left: 50px;">

- Go to `Developer --> Chain State` and select `assets --> account` with the proposal target account and press `+` button

- From the response payload we might see that 45000 tokens have been transferred since the proposal has been approved

<img src="images/assets/Screenshot%202023-02-06%20at%2015.29.30.png" width="300" style="padding-left: 50px;">

- Repeat steps [Proposal Submission](#proposal-submission) and below with another account as a target

## Democracy Proposals

### Proposal Submission

Now let's try to make use of a DAO Governance token via DAO Democracy Pallet:

- Go to `Developer --> Exstrinsic` and select `daoDemocracy --> propose`

- Set the following parameters for a call:

```
daoId: 0
```

- For the proposal call use the following:

```
proposal: Inline
inline: 0x30000000000090b5ab205c6974c9ea841be688864633dc9ca8a357843eeacf2314649965fe22
value: 1000
```

Use the approach described in [Retrieve Proposal Call Data/Hash](./EthereumTestingGuide.md#retrieve-proposal-call-datahash) to get proposal encoded data

`NOTE: In this particular case we're using the call data of dao membership pallet call to add a new member`

<img src="images/assets/Screenshot%202023-02-06%20at%2018.02.18.png" width="600" style="padding-left: 50px;">

- `Submit Transaction`

- Go to `Developer --> Chain State` and select `assets --> account` with dao token as asset id and proposer account as parameters

- Observe that token balance decrease on the amount of tokens deposited (in our case it is 1000 tokens) moving into reserved state:

<img src="images/assets/Screenshot%202023-02-06%20at%2015.45.05.png" width="300" style="padding-left: 50px;">

## Referendum

### Launch Period

- Now wait for about 5 minutes until the proposal becomes a referendum

- Check `daoDemocracy --> referendumCount` value until it becomes 1:

<img src="images/assets/Screenshot%202023-02-06%20at%2015.48.02.png" width="300" style="padding-left: 50px;">

### Voting

- Go to `Developer --> Extrinsics` and select `daoDemocracy --> vote`

- Set the following parameters for a call:

```
daoId: 0
refIndex: 0
vote: Standard
aye: Aye
conviction: Conviction
balance: 40000
```

<img src="images/assets/Screenshot%202023-02-06%20at%2015.49.17.png" width="600" style="padding-left: 50px;">

- `Submit Transaction`

- Go to `Developer --> Chain State` and select `assets --> account` with dao token and voter account as parameters

- Observe that amount of tokens that has been for voting is now in `frozenBalance` state:

<img src="images/assets/Screenshot%202023-02-06%20at%2015.49.29.png" width="300" style="padding-left: 50px;">

### Transfer Frozen Tokens

- Go to `Developer --> Extrinsics` and select `asset --> transfer`

- Set the following parameters for a call:

```
id: 1
target: <select account>
amount: 40000
```

<img src="images/assets/Screenshot%202023-02-06%20at%2016.08.07.png" width="600" style="padding-left: 50px;">

- `Submit Transaction`

- Observe 'BalanceLow' error occurs

<img src="images/assets/Screenshot%202023-02-06%20at%2016.08.15.png" width="300" style="padding-left: 50px;">

### Referendum execution

- Wait ~5 minutes until the referendum finishes

- The Referendum status should go into `Approved` state:

<img src="images/assets/Screenshot%202023-02-06%20at%2018.04.35.png" width="300" style="padding-left: 50px;">

- It will be executed in ~5 minutes after finish

### Remove Referendum vote

- Go to `Developer --> Extrinsics` and select `daoDemocracy --> removeVote`

- Set the following parameters for a call:

```
id: 1
daoId: 0
index: 0
```

- `Submit Transaction`

### Unlock tokens

- Go to `Developer --> Extrinsics` and select `daoDemocracy --> unlock`

- Set the following parameters for a call:

```
id: 1
daoId: 0
target: <select account you want the tokens to be unlocked for>
```

- `Submit Transaction`

- Go to `Developer --> Chain State` and select `assets --> account` with dao token and account that had `frozenBalance` as parameters

- Observe that `frozenBalance` is set 0

### Verify tokens unlocked

- Go to `Developer --> Extrinsics` and select `asset --> transfer`

- Set the following parameters for a call:

```
id: 1
target: <select account>
amount: 40000
```

<img src="images/assets/Screenshot%202023-02-06%20at%2016.08.07.png" width="600" style="padding-left: 50px;">

- `Submit Transaction`

- No errors observed - tokens transferred successfully.
