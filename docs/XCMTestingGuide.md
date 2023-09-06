# Societal Node Cumulus/XCM Testing Guide

## Launch Rococo Local Testnet

- Please refer to the [Run Local Rococo Testnet with Societal Node Parachains](../README.md#run-rococo-local-testnet) section of the README

## Register Societal Parachain - Para ID 2000

- Connect to Alice validator Relay Chain: `https://cloudflare-ipfs.com/ipns/dotapps.io/?rpc=ws%3A%2F%2Flocalhost%3A9944#/explorer`

- Go to `Developer --> Extrinsics` section and select `parasSudoWrapper --> sudoScheduleParaInitialize(id, genesis)`

- Use `2000` as parachain `id` and set `parachain` to `true.

- Use [para-2000-genesis-state](../examples/para-2000-genesis-state) file as `genesisHead`

- Use [para-2000-wasm](../examples/para-2000-wasm) file as `validationCode`

- Go to `Network --> Parachains` section and wait for ~2 minutes to check whether the 2000 chain is in the `Parachains` list

-  Connect to Societal Parachain(2000): `https://cloudflare-ipfs.com/ipns/dotapps.io/?rpc=ws%3A%2F%2Flocalhost%3A9954#/explorer`

- Go to `Network --> Explorer` and check that blocks are produced and finalized

## Register 2nd Parachain - Para ID 2001

- Basically the same steps as in previous steps but different genesis/wasm files taking part

- Connect to Alice validator Relay Chain: `https://cloudflare-ipfs.com/ipns/dotapps.io/?rpc=ws%3A%2F%2Flocalhost%3A9944#/explorer`

- Go to `Developer --> Extrinsics` section and select `parasSudoWrapper --> sudoScheduleParaInitialize(id, genesis)`

- Use `2001` as parachain `id` and set `parachain` to `true.

- Use [para-2001-genesis-state](../examples/para-2001-genesis-state) file as `genesisHead`

- Use [para-2001-wasm](../examples/para-2001-wasm) file as `validationCode`

- Go to `Network --> Parachains` section and wait for ~2 minutes to check whether the 2001 chain is in the `Parachains` list

-  Connect to Societal Parachain(2000): `https://cloudflare-ipfs.com/ipns/dotapps.io/?rpc=ws%3A%2F%2Flocalhost%3A9955#/explorer`

- Go to `Network --> Explorer` and check that blocks are produced and finalized

## Open HRMP Channel from Parachain 2000 to 2001

- Refer to the [Open message passing channels](https://docs.substrate.io/tutorials/build-a-parachain/open-message-passing-channels/) section of Polkadot documentation

- Use the following sovereign accounts for Parachains connected:

```
ParaId 2000 - 5Ec4AhPUwPeyTFyuhGuBbD224mY85LKLMSqSSo33JYWCazU4
ParaId 2001 - 5Ec4AhPV91i9yNuiWuNunPf6AQCYDhFTTA4G5QCbtqYApH9E
```

- Please make sure you open bi-directional HRMP channel completing the guide from the previous step for both 2000 and 2001 parachains.

- Check whether the HRMP channels are opened in both directions:

<img src="images/xcm/Screenshot 2023-04-18 at 16.43.43.png" width="600" style="padding-left: 50px;">

- Make sure assets can be transferred via HRMP channel using the Polkadot tutorial doc: [Transfer assets with XCM](https://docs.substrate.io/tutorials/build-a-parachain/transfer-assets-with-xcm/)
