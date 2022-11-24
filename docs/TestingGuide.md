
# Societal Node Testing Guide

## Create a DAO

- Open Polkadot JS UI

- Connect to `development node --> local node`

<img src="images/Screenshot%202022-11-18%20at%2015.41.10.png" width="200" style="padding-left: 50px;">

- Open `Developer --> Extrinsics` section

- From the list of available extrinsics select `dao --> createDao` callable

<img src="images/Screenshot%202022-11-18%20at%2011.50.10.png" width="600" style="padding-left: 50px;">

- Select accounts you want to be a part of the council of the DAO

- Use the following json as an example of a DAO data payload:

```json
{
  "name": "name",
  "purpose": "purpose",
  "metadata": "metadata",
  "policy": {
    "proposal_bond": 1,
    "proposal_bond_min": 1,
    "proposal_period": 300000,
    "approve_origin": [
      1,
      2
    ],
    "reject_origin": [
      1,
      2
    ]
  },
  "token": {
    "token_id": 1,
    "min_balance": "1000000",
    "metadata": {
      "name": "token",
      "symbol": "symbol",
      "decimals": 10
    }
  }
}
```

- `Submit Transaction` --> DAO successfully created

- Go to `Developer --> Chain State`

- Select `dao --> daos` for state query and press `+` button

- Now you can see the basic data of your DAO

<img src="images/Screenshot%202022-11-18%20at%2012.12.05.png" width="600" style="padding-left: 50px;">

- Copy the `account_id` from the response - it will be later used for DAO treasury 

- You can select `dao --> policies` to query DAO policy settings

<img src="images/Screenshot%202022-11-18%20at%2012.13.36.png" width="600" style="padding-left: 50px;">

- Select `system --> account` and

- Paste the `account_id` of your DAO from the previous step and press `+` button

- You can see that DAO has some free balance - it was added while creating a DAO

<img src="images/Screenshot%202022-11-18%20at%2012.14.42.png" width="600" style="padding-left: 50px;">

- Select `daoCouncil --> members` and press `+` button

- You can observe the council members of the DAO that's been selected while creating

<img src="images/Screenshot%202022-11-18%20at%2012.16.00.png" width="600" style="padding-left: 50px;">

- Select `assets --> asset` from query list

- Use `token_id=1` since we used it while creating the DAO and press `+` button

- You can observe that a new token owned by the DAO was created 

<img src="images/Screenshot%202022-11-18%20at%2012.21.59.png" width="600" style="padding-left: 50px;">

## Treasury Proposals

### Proposal Submission

Now let's try to create a proposal to spend some DAO money

- Go to `Developer --> Extrinsics` and select `daoTreasury --> proposeSpend`

<img src="images/Screenshot%202022-11-18%20at%2012.25.28.png" width="600" style="padding-left: 50px;">

- Use the following parameters for the proposal:

```
daoId: 0
value: 500
beneficiary: select account from the dropdown
```

- `Submit Transaction`

- Go to `Developer --> Extrinsics` and select `daoCouncil --> propose`

- Use the following parameters for the council proposal:

```
daoId: 0
threshold: 2
```

- For the proposal call use the following:

```
proposal: daoTreasury --> approveProposal
daoId: 0
proposalId: 0
lengthBound: 100
```

<img src="images/Screenshot%202022-11-18%20at%2012.36.05.png" width="600" style="padding-left: 50px;">

- `Submit Transaction`

- Go to `Developer --> Chain State`

- Select `daoCouncil --> proposals` and press `+` button

<img src="images/Screenshot%202022-11-18%20at%2012.41.09.png" width="600" style="padding-left: 50px;">

- Copy the proposal hash from the response payload

### Proposal Voting

- Go to `Developer --> Extrinsics` and select `daoCouncil --> vote`

<img src="images/Screenshot%202022-11-18%20at%2012.42.04.png" width="600" style="padding-left: 50px;">

- Submit Transaction

- Use different accounts(2 overall) to vote `yes` to overcome the approval threshold

- Try to use the account that is not in council to check that its vote is rejected

<img src="images/Screenshot%202022-11-18%20at%2013.10.18.png" width="200" style="padding-left: 50px;">

- To close the proposal prematurely before the end time select `daoCouncil --> close` with the following parameters

```
daoId: 0
proposalHash: <proposal hash from the steps above>
proposalIndex: 0
proposalWeightBound: 1000000000
lengthBound: 100
```

<img src="images/Screenshot%202022-11-18%20at%2013.12.48.png" width="600" style="padding-left: 50px;">

- Go to `Developer --> Chain State` and select `system --> account` with the DAO Account and press `+` button

- From the response payload we might see that 500 tokens from the DAO Treasury pot have been spent since the proposal has been approved

<img src="images/Screenshot%202022-11-18%20at%2013.15.36.png" width="600" style="padding-left: 50px;">


## Membership Proposals

### Proposal Submission

Now let's try to manage council DAO membership via DAO Membership Pallet

- Go to `Developer --> Exstrinsic` and select `daoCouncil --> propose`

- Set the following parameters for a call:

```
daoId: 0
threshold: 2
```

- For the proposal call use the following:

```
proposal: daoCouncilMemberships --> removeMember
daoId: 0
who: select account from council(e.g. BOB)
lengthBound: 100
```

<img src="images/Screenshot%202022-11-18%20at%2016.27.53.png" width="600" style="padding-left: 50px;">

- Perform the voting similar to the treasury proposal approval from above. Use increased proposal index(1 for this case) and proposal hash from chain state: `Developer --> Chain State` and select `daoCouncil --> proposals` and press `+` button:

- After successful voting and closing the proposal go to `Developer --> Chain State`

- select `daoCouncil --> members` and press `+` button 

<img src="images/Screenshot%202022-11-18%20at%2013.55.23.png" width="600" style="padding-left: 50px;">

- Observing the result you can see that BOB account was removed from the council
