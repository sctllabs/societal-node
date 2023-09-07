#!/usr/bin/env bash
# This script is meant to be run on Unix/Linux based systems
set -e

echo "*** Start Rococo Local Testnet ***"

cd $(dirname ${BASH_SOURCE[0]})/..

docker-compose -f ./docker-compose.yml down --remove-orphans
docker-compose -f ./docker-compose.yml up -d
