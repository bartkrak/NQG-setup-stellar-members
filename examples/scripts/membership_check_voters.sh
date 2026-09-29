#!/bin/bash
# Checks that every voter id in data/voters.json is an active member of the
# Stellar Membership contract. The governance contract rejects neuron results
# for any other id with NotAMember, so run this before uploading.
ENV_PATH=".env"
source $ENV_PATH

VOTERS_FILE="./data/voters.json"
FAILED=0

for token_id in $(jq -r '.[]' "$VOTERS_FILE"); do
  if OWNER=$(stellar contract invoke \
    --id $MEMBERSHIP_CONTRACT_ADDRESS \
    --source-account $STELLAR_SECRET_KEY \
    --rpc-url $STELLAR_RPC_URL \
    --network-passphrase "$STELLAR_NETWORK_PASSPHRASE" \
    --send=no \
    -- owner_of \
    --token_id $token_id 2>/dev/null); then
    echo "$token_id active, owned by $OWNER"
  else
    echo "$token_id NOT an active member (never minted or revoked)"
    FAILED=1
  fi
done

exit $FAILED
