
# Societal Node Testing Guide - Integration with Ethereum(Metamask)

## Build and Run Node

- Build the node with the following command:

```
cargo b --release
```

- Run node in dev mode so everytime you restart all the data is cleared and it starts from block 0:

```
./target/release/societal-node --dev
```

## Setup Metamask

- Click on `Accounts --> Settings` and select `Networks`

- Click on `Add a network` and use the following settings to connect to the local Societal Node:

<img src="images/metamask/Screenshot%202022-12-12%20at%2021.26.42.png" width="600" style="padding-left: 50px;">

- Save settings

- Click on 'Accounts --> Import Account'

- Use following credentials to import 'Gerald' account:

```
Public Address: 0x6Be02d1d3665660d22FF9624b7BE0551ee1Ac91b
Private Key: 0x99b3c12287537e38c90a9219d4cb074a89a16e9cdb20bf85728ebd97c343e342
```

- New account should appear in the list of accounts with - it should have some SCTL tokens:

<img src="images/metamask/Screenshot%202022-12-12%20at%2021.41.24.png" width="600" style="padding-left: 50px;">

- Make sure you reset the account after each node restart:

- In Metamask select `Account --> Settings --> Advanced` and click `Reset Account`:

<img src="images/metamask/Screenshot%202022-12-13%20at%2010.26.42.png" width="600" style="padding-left: 50px;">


- Make sure you reset all the temporary accounts you have connected to the local node

## Setup Remix IDE

- Open `https://remix.ethereum.org/`

- Go to `contracts` section and create 4 files based on the interfaces defined in `precompiles` section of this repo:

- `Dao.sol` - `precompiles/dao/Dao.sol`
- `DaoTreasury.sol` - `precompiles/dao-treasury/DaoTreasury.sol`
- `DaoCollective.sol` - `precompiles/dao-collective/DaoCollective.sol`
- `DaoMembership.sol` - `precompiles/dao-membership/DaoMembership.sol`

- Your IDE setup should look like this:

<img src="images/metamask/Screenshot%202022-12-12%20at%2021.46.45.png" width="600" style="padding-left: 50px;">

- Go to `Deploy & run transactions` section of the Remix IDE

- In the `Environment` dropdown select `Injected Provider - Metamask`

- Connect to your Metamask wallet and select required accounts:

<img src="images/metamask/Screenshot%202022-12-12%20at%2021.50.23.png" width="600" style="padding-left: 50px;">

- Select `PalletDao` as a contract

- Paste `0x0000000000000000000000000000000000000006` into the `Load Contract from Address` input and click `At Address` button:

- You can observe that you're able to call transactions using the `Dao.sol` inteface functions:

<img src="images/metamask/Screenshot%202022-12-12%20at%2021.54.35.png" width="600" style="padding-left: 50px;">

## Create a DAO

- In the `council` field put the account addresses taken from the Metamask wallet:

```
[ "0x6Be02d1d3665660d22FF9624b7BE0551ee1Ac91b", "0x71712eE6E0445A079d9aa41f3e327dD3d9CA5f38" ]
```

- Where `0x6Be02d1d3665660d22FF9624b7BE0551ee1Ac91b` is the Gerald account and the second one can be any account from the Metamask wallet

- Use the encoded json data as the DAO payload:

```
0x7B0A2020226E616D65223A20226E616D65222C0A202022707572706F7365223A2022707572706F7365222C0A2020226D65746164617461223A20226D65746164617461222C0A202022706F6C696379223A207B0A202020202270726F706F73616C5F626F6E64223A20312C0A202020202270726F706F73616C5F626F6E645F6D696E223A20312C0A202020202270726F706F73616C5F706572696F64223A203330303030302C0A2020202022617070726F76655F6F726967696E223A205B0A202020202020312C0A202020202020320A202020205D2C0A202020202272656A6563745F6F726967696E223A205B0A202020202020312C0A202020202020320A202020205D0A20207D2C0A202022746F6B656E223A207B0A2020202022746F6B656E5F6964223A20312C0A20202020226D696E5F62616C616E6365223A202231303030303030222C0A20202020226D65746164617461223A207B0A202020202020226E616D65223A2022746F6B656E207364736466222C0A2020202020202273796D626F6C223A202273796D626F6C2020736466736466222C0A20202020202022646563696D616C73223A2031300A202020207D0A20207D0A7D
```

- The transaction form should look like this:

<img src="images/metamask/Screenshot%202022-12-12%20at%2022.00.10.png" width="300" style="padding-left: 50px;">

- Click `transact` and sign transaction in Metamask wallet popup(make sure you use the account with some funds)

- You should see the following logs after transaction is signed and submitted:

<img src="images/metamask/Screenshot%202022-12-12%20at%2022.02.14.png" width="600" style="padding-left: 50px;">

- Check the DAO information using the PolkadotJS UI described here:

[Observe DAO Information](./TestingGuide.md#observe-dao-information)

## Treasury Proposals

### Proposal Submission

Now let's try to create a proposal to spend some DAO money

- In Remix IDE select `DaoTreasury.sol` and use `0x0000000000000000000000000000000000000007` as the address for implementation

- Use `proposeSpend` section with the following parameters(use any available account from the wallet as the beneficiary):

```
dao_id: 0
value: 99
beneficiary: 0x71712eE6E0445A079d9aa41f3e327dD3d9CA5f38
```

<img src="images/metamask/Screenshot%202022-12-12%20at%2022.16.55.png" width="300" style="padding-left: 50px;">

- Click `transact` and sign transaction. Make sure it succeeded following the logs in the Remix IDE.

- Select `DaoCollective.sol` and use `0x0000000000000000000000000000000000000008` as the address for implementation
  
- Use `propose` with the following parameters:

```
dao_id: 0
threshold: 2
proposal: 0x15020000
```

<img src="images/metamask/Screenshot%202022-12-12%20at%2022.26.33.png" width="600" style="padding-left: 50px;">

### Retrieve Proposal Call Data/Hash

- To get the proposal encoded call used above data go to `PolkadotJS` UI

- Go to `Developer --> Extrinsics` and select `daoTreasury --> approveProposal`

- Use the following parameters for the call:

```
daoId: 0
proposalId: 0
```

- Copy `encoded call data` or `encoded call hash`(if you need a hash) below the extrinsic submision form:

<img src="images/metamask/Screenshot%202022-12-12%20at%2022.30.32.png" width="600" style="padding-left: 50px;">

- Go back to Remix IDE and submit the transaction

- Let's get proposals from dao-collective pallet

- Select `proposals` function in the `DaoCollective` contract section with the `dao_id: 0` and click `call`:

- You can see that proposal exists in the system

<img src="images/metamask/Screenshot%202022-12-12%20at%2022.47.22.png" width="300" style="padding-left: 50px;">

### Proposal Hash Retrieval

- Before starting to vote on the proposal let's retrieve the hash of it

- Go to `proposalHash` function in the `DaoCollective` contract section and use proposal from the previous step `0x15020000`

- Click `call` and you should see the hash code of the proposal:

<img src="images/metamask/Screenshot%202022-12-12%20at%2022.34.58.png" width="300" style="padding-left: 50px;">

- Copy proposal hash from the call response - it will be used in the next steps

### Proposal Voting

- Use `vote` function call in the `DaoCollective` contract section with the following parameters:

```
dao_id: 0
proposalHash: 0x068fc625d098e6e5cb71c3b051691971ef62b6018032d1c5a0f2846f0a582786
proposalIndex: 0
approve: true
```

<img src="images/metamask/Screenshot%202022-12-12%20at%2022.38.02.png" width="300" style="padding-left: 50px;">

- Click `transact` and sign transaction - make sure the transaction succeeded

- Go to Metamask wallet and switch accounts - select the account that was used as the `council` while creating a DAO. Make sure it has
some positive token balance - you can always transfer some tokens from Gerald account.

- You should see that account has also changes in the Remix IDE after it was switched in Metamask:

<img src="images/metamask/Screenshot%202022-12-12%20at%2022.43.15.png" width="300" style="padding-left: 50px;">

- Click `transact` and sign transaction - make sure the transaction succeeded

- Select `close` function in the `DaoCollective` contract section with the following parameters:

```
dao_id: 0
proposalHash: 0x068fc625d098e6e5cb71c3b051691971ef62b6018032d1c5a0f2846f0a582786
proposalIndex: 0
proposalWeightBound: 10000
lengthBound: 10000
```

- Click `transact` and sign transaction - make sure the transaction succeeded

- Let's check that proposal has been voted through and removed from the system

- Select `proposals` function in the `DaoCollective` contract section with the `dao_id: 0` and click `call`:

- You can see that proposal response is empty

- However `dao-treasury` pallet hasn't yet completed the spend proposal. The reason is that it doesn't have enough funds to proceed with it. 

- Now let's check the balance of the DAO

- Go to `PolkadotJS` UI and select `Developer --> Chain State`

- Select `system --> account` extrinsic with the following parameters:

```
AccountId: 5EYCAe5ijiYfqMFxyJDmHoxzF1VJ4NTsqtRAgdjN3q6pCz51 (DAO Account ID)
```

- You should get the following response:

```json
{
  nonce: 0
  consumers: 1
  providers: 1
  sufficients: 0
  data: {
    free: 500
    reserved: 0
    miscFrozen: 0
    feeFrozen: 0
  }
}
```

- The `free` here is the balance that DAO can spend. It is set to minimum existing balance property of the blockchain to make
sure the DAO account exists in the system. So we need to transfer some tokens to DAO to complete the proposal.

- Go to `Developer --> Extrinsics` and select `balances --> transfer`

- Use the DAO Account ID from above as the destination and send 100 tokens to it. Submit transaction.

- Check the account balance using `Developer --> Chain State --> system --> account`:
  
<img src="images/metamask/Screenshot%202022-12-13%20at%2010.37.24.png" width="600" style="padding-left: 50px;">

- After some time you should notice that the amount of `free` tokens has dropped on 99 token(number of tokens spend by the approved proposal)

<img src="images/metamask/Screenshot%202022-12-13%20at%2010.37.43.png" width="600" style="padding-left: 50px;">

- If you go to `Network --> Explorer` section you can observe event from `daoTreasury` pallet called `Awarded`:

<img src="images/metamask/Screenshot%202022-12-13%20at%2010.40.37.png" width="300" style="padding-left: 50px;">

## Membership Proposals

### Proposal Submission

Now let's try to manage council DAO membership via DAO Membership Pallet

- In Remix IDE select `DaoCollective.sol` and use `0x0000000000000000000000000000000000000007` as the address for implementation

- Use `propose` section with the following parameters(use any available account from the wallet as the beneficiary):

```
dao_id: 0
threshold: 2
proposal: 0x2f01000000003aecc9734e2c191a9894894439a66fa6283d9cac82eff135a013a60881c776d0
```

- Use the approach described in [Retrieve Proposal Call Data/Hash](#retrieve-proposal-call-datahash) to get proposal encoded data

- Click `transact` and sign transaction - make sure the transaction succeeded

- Use `vote` function call in the `DaoCollective` contract section with the following parameters:

```
dao_id: 0
proposalHash: 0x068fc625d098e6e5cb71c3b051691971ef62b6018032d1c5a0f2846f0a582786
proposalIndex: 1
approve: true
```

- Vote by the second account in the council group to go above the threshold defined

- Select `close` function in the `DaoCollective` contract section with the following parameters:

```
dao_id: 0
proposalHash: 0x068fc625d098e6e5cb71c3b051691971ef62b6018032d1c5a0f2846f0a582786
proposalIndex: 1
proposalWeightBound: 100000000000
lengthBound: 10000
```

- Select `isMember` function in the `DaoCollective` contract and call it with the following parameters:

```
dao_id: 0
account: 0x71712eE6E0445A079d9aa41f3e327dD3d9CA5f38
```

- Check that result of the call is `bool: false`. That means that council member has been removed after voting completed.
