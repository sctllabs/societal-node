#!/usr/bin/env bash
# This script is meant to be run on Unix/Linux based systems
set -e

echo "*** Deploying Rococo Local Testnet ***"

# Alice Validator
helm \
  -n rococo-dev \
  upgrade \
  rococo-alice \
  --install \
  ../helm-charts/charts/node \
  -f ./deployment/rococo-values.yaml \
  --set node.role="validator" \
  --set node.customNodeKey="91cb59d86820419075b08e3043cd802ba3506388d8b161d2d4acd203af5194c1" \
  --set node.perNodeServices.relayP2pService.enabled=true \
  --set node.perNodeServices.relayP2pService.port=30333 \
  --set "node.flags[0]=--alice"

# Bob Validator
helm \
  -n rococo-dev \
  upgrade \
  rococo-bob \
  --install \
  ../helm-charts/charts/node \
  -f ./deployment/rococo-values.yaml \
  --set node.role="validator" \
  --set "node.flags[0]=--bob" \
  --set "node.flags[1]=--bootnodes" \
  --set "node.flags[2]='/dns4/rococo-alice-node-0-relay-chain-p2p/tcp/30333/p2p/12D3KooWJHhnF64TXSmyxNkhPkXAHtYNRy86LuvGQu1LTi5vrJCL'"

# Charlie Validator
helm \
  -n rococo-dev \
  upgrade \
  rococo-charlie \
  --install \
  ../helm-charts/charts/node \
  -f ./deployment/rococo-values.yaml \
  --set node.role="validator" \
  --set "node.flags[0]=--charlie" \
  --set "node.flags[1]=--bootnodes" \
  --set "node.flags[2]='/dns4/rococo-alice-node-0-relay-chain-p2p/tcp/30333/p2p/12D3KooWMeR4iQLRBNq87ViDf9W7f6cc9ydAPJgmq48rAH116WoC'"

echo "*** Deploying Rococo Testnet Node Pool ***"

# Node Pool - not required
helm \
  -n rococo-dev \
  upgrade \
  rococo-pool \
  --install \
  ../helm-charts/charts/node \
  -f ./deployment/rococo-values.yaml \
  --set node.replicas=2 \
  --set "node.flags[0]=--bootnodes" \
  --set "node.flags[1]='/dns4/rococo-alice-node-0-relay-chain-p2p/tcp/30333/p2p/12D3KooWMeR4iQLRBNq87ViDf9W7f6cc9ydAPJgmq48rAH116WoC'"

echo "*** Deploying Societal Parachain 2000 ***"

# Societal Parachain 2000
helm \
  -n rococo-dev \
  upgrade \
  societal-node \
  --install \
  ../helm-charts/charts/node \
  -f ./deployment/parachain-values.yaml \
  --set "node.collatorRelayChain.flags[0]=--bootnodes" \
  --set "node.collatorRelayChain.flags[1]='/dns4/rococo-alice-node-0-relay-chain-p2p/tcp/30333/p2p/12D3KooWMeR4iQLRBNq87ViDf9W7f6cc9ydAPJgmq48rAH116WoC'"

echo "*** Deploying Societal Parachain 2001 ***"

# Societal Parachain 2001
helm \
  -n rococo-dev \
  upgrade \
  societal-parachain-node \
  --install \
  ../helm-charts/charts/node \
  -f ./deployment/parachain-2001-values.yaml \
  --set "node.collatorRelayChain.flags[0]=--bootnodes" \
  --set "node.collatorRelayChain.flags[1]='/dns4/rococo-alice-node-0-relay-chain-p2p/tcp/30333/p2p/12D3KooWMeR4iQLRBNq87ViDf9W7f6cc9ydAPJgmq48rAH116WoC'"
