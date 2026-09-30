#!/bin/bash
# Checks that every voter id in data/voters.json is an active member of the
# Stellar Membership contract. The governance contract rejects neuron results
# for any other id with NotAMember, so run this before uploading.
ENV_PATH="${ENV_PATH:-.env}"
source $ENV_PATH

VOTERS_FILE="./data/voters.json"
FAILED=0

# member() fails for a token never minted; status is 0 Active, 1 Revoked.
for token_id in $(jq -r '.[]' "$VOTERS_FILE"); do
  STATUS=$(stellar contract invoke \
    --id $MEMBERSHIP_CONTRACT_ADDRESS \
    --source-account $STELLAR_SECRET_KEY \
    --rpc-url $STELLAR_RPC_URL \
    --network-passphrase "$STELLAR_NETWORK_PASSPHRASE" \
    --send=no \
    -- member \
    --token_id $token_id 2>/dev/null | jq -r '.status')
  case "$STATUS" in
    0) echo "$token_id active" ;;
    1) echo "$token_id NOT active: revoked"; FAILED=1 ;;
    *) echo "$token_id NOT active: never minted"; FAILED=1 ;;
  esac
done

exit $FAILED
