#!/usr/bin/env bash
# This script is meant to be run on Unix/Linux based systems
set -e

echo "*** Start Rococo Local Testnet with Societal Node Parachains ***"

cd $(dirname ${BASH_SOURCE[0]})/..

docker-compose -f ./docker-compose-rococo-local-testnet.yml down --remove-orphans
docker-compose -f ./docker-compose-rococo-local-testnet.yml up -d
