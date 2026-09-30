#!/bin/bash
# Reads the voting powers (NQG scores) calculated for the current round
ENV_PATH="${ENV_PATH:-.env}"
source $ENV_PATH

VOTERS_FILE="./data/voters.json"

echo "All voting powers (token id -> power)"
stellar contract invoke \
  --id $NEURAL_GOVERNANCE_ADDRESS \
  --source-account $STELLAR_SECRET_KEY \
  --rpc-url $STELLAR_RPC_URL \
  --network-passphrase "$STELLAR_NETWORK_PASSPHRASE" \
  --send=no \
  -- get_voting_powers

echo "Voting power per member"
for member_id in $(jq -r '.[]' "$VOTERS_FILE"); do
  POWER=$(stellar contract invoke \
    --id $NEURAL_GOVERNANCE_ADDRESS \
    --source-account $STELLAR_SECRET_KEY \
    --rpc-url $STELLAR_RPC_URL \
    --network-passphrase "$STELLAR_NETWORK_PASSPHRASE" \
    --send=no \
    -- get_voting_power_for_user \
    --member_id $member_id)
  echo "$member_id $POWER"
done
