#!/bin/bash
# End-to-end test of NQG with the Stellar Membership contract on testnet.
#
# Deploys a fresh membership contract (not the shared testnet one), mints the
# members from data/voters.json, runs the example scripts to deploy NQG and
# compute voting powers, checks them directly and through the membership
# contract, then revokes a member and checks NQG rejects it.
#
# Usage, from examples/:
#   MEMBERSHIP_WASM=../../stellar-membership/target/wasm32v1-none/release/stellar_membership.wasm \
#     ./scripts/e2e_testnet.sh
#
# Your .env is not used: the example scripts get a temporary config.

set -uo pipefail

# E2E_* overrides allow a dry run on a local network.
NETWORK="${E2E_NETWORK:-testnet}"
RPC_URL="${E2E_RPC_URL:-https://soroban-testnet.stellar.org}"
PASSPHRASE="${E2E_PASSPHRASE:-Test SDF Network ; September 2015}"
ROUND=1

# The stellar CLI loads .env from this or any parent folder, and its
# STELLAR_RPC_URL silently overrides --network. Exported vars win over it.
export STELLAR_NETWORK="$NETWORK"
export STELLAR_RPC_URL="$RPC_URL"
export STELLAR_NETWORK_PASSPHRASE="$PASSPHRASE"
export STELLAR_ACCOUNT=""

KEY_PREFIX="nqg-e2e"

REVOKED_ID=2

MEMBERSHIP_WASM="${MEMBERSHIP_WASM:-}"
VOTERS_FILE="./data/voters.json"
NEURONS_FILE="./data/neurons_output.json"

PASSED=0
FAILED=0

step() { echo; echo "=== $*"; }
pass() { echo "PASS: $*"; PASSED=$((PASSED + 1)); }
fail() { echo "FAIL: $*"; FAILED=$((FAILED + 1)); }

abort() { echo "ABORT: $*"; exit 1; }

# send <signer> <contract> <fn> [args...]
send() {
  local who=$1 id=$2
  shift 2
  stellar contract invoke --network $NETWORK --source-account "$who" --id "$id" -- "$@"
}

# read_only <contract> <fn> [args...]
read_only() {
  local id=$1
  shift
  stellar contract invoke --network $NETWORK --source-account "$KEY_PREFIX-admin" \
    --id "$id" --send=no -- "$@" 2>/dev/null
}

unquote() { tr -d '"'; }

step "0. Preconditions"

[ -n "$MEMBERSHIP_WASM" ] || abort "set MEMBERSHIP_WASM to the membership contract .wasm (see the header of this script)"
[ -f "$MEMBERSHIP_WASM" ] || abort "MEMBERSHIP_WASM not found: $MEMBERSHIP_WASM"
[ -f "$VOTERS_FILE" ] || abort "run this script from the examples/ folder"
for tool in stellar jq bc; do
  command -v $tool >/dev/null || abort "$tool is not installed"
done

VOTERS=$(jq -r '.[]' "$VOTERS_FILE")
echo "Voters from $VOTERS_FILE: $(echo $VOTERS)"
echo "Membership WASM: $MEMBERSHIP_WASM"

step "1. Create and fund testnet identities"

MEMBER_KEYS=()
for id in $VOTERS; do
  MEMBER_KEYS+=("$KEY_PREFIX-m$id")
done

for key in "$KEY_PREFIX-admin" "$KEY_PREFIX-mem-admin" "${MEMBER_KEYS[@]}"; do
  if stellar keys address "$key" >/dev/null 2>&1; then
    stellar keys fund "$key" --network $NETWORK >/dev/null 2>&1
    echo "$key reused: $(stellar keys address $key)"
  else
    stellar keys generate "$key" --network $NETWORK --fund >/dev/null 2>&1 \
      || abort "could not create $key"
    echo "$key created: $(stellar keys address $key)"
  fi
done

# NQG does not exist yet: deploy with a placeholder NQG address, fixed in
# step 6. mint needs both the member's and an operator's auth, and the CLI
# signs only for the source account, so each member is its own operator.

step "2. Deploy a fresh membership contract"

OPERATORS=$(for key in "${MEMBER_KEYS[@]}"; do stellar keys address "$key"; done | jq -R . | jq -s -c .)

MEMBERSHIP=$(stellar contract deploy --network $NETWORK --source-account "$KEY_PREFIX-mem-admin" \
  --wasm "$MEMBERSHIP_WASM" \
  -- \
  --admin "$KEY_PREFIX-mem-admin" \
  --operators "$OPERATORS" \
  --name "NQG e2e test" --symbol TEST \
  --uri https://example.com --uri_trait https://example.com \
  --nqg_contract "$KEY_PREFIX-mem-admin" 2>/dev/null)
[ -n "$MEMBERSHIP" ] || abort "membership deploy failed"
echo "Membership contract: $MEMBERSHIP"

step "3. Mint one member per voter id"

# A fresh contract mints ids 0, 1, 2, ..., which must match voters.json.
for id in $VOTERS; do
  key="$KEY_PREFIX-m$id"
  MINTED=$(send "$key" "$MEMBERSHIP" mint \
    --to "$key" --operator "$key" --role 0 \
    --external_accounts '{"accounts":[],"email_hash":null}' \
    --bio="e2e member $id" --projects '[]' 2>/dev/null)
  if [ "$MINTED" = "$id" ]; then
    pass "minted token $id"
  else
    abort "expected token id $id, mint returned '$MINTED' (voters.json must be 0, 1, 2, ... for a fresh contract)"
  fi
done

step "4. Deploy NQG and compute voting powers with the example scripts"

# Config for the example scripts; governance_deploy.sh adds the NQG address.
E2E_DIR=$(mktemp -d)
export ENV_PATH="$E2E_DIR/e2e.env"
cat > "$ENV_PATH" <<EOF
STELLAR_PUBLIC_KEY=$(stellar keys address $KEY_PREFIX-admin)
STELLAR_SECRET_KEY=$(stellar keys secret $KEY_PREFIX-admin)
CURRENT_ROUND=$ROUND
STELLAR_RPC_URL="$RPC_URL"
STELLAR_NETWORK_PASSPHRASE="$PASSPHRASE"
STELLAR_NETWORK="$NETWORK"
NEURAL_GOVERNANCE_ADDRESS=
MEMBERSHIP_CONTRACT_ADDRESS=$MEMBERSHIP
EOF
echo "Config for the example scripts: $ENV_PATH"

if ./scripts/membership_check_voters.sh; then
  pass "every voter is an active member"
else
  fail "membership_check_voters.sh reported an inactive voter"
fi

./scripts/governance_deploy.sh >/dev/null 2>&1
NQG=$(grep '^NEURAL_GOVERNANCE_ADDRESS=' "$ENV_PATH" | cut -d= -f2)
[ -n "$NQG" ] || abort "NQG deploy failed (run ./scripts/governance_deploy.sh by hand with ENV_PATH=$ENV_PATH to see why)"
echo "NQG contract: $NQG"

UPLOAD_OUTPUT=$(./scripts/governance_upload_neurons_results.sh 2>&1)
UPLOADS_OK=$(echo "$UPLOAD_OUTPUT" | grep -c "submitted successfully")
if [ "$UPLOADS_OK" -eq 3 ]; then
  pass "uploaded the results of 3 neurons"
else
  echo "$UPLOAD_OUTPUT"
  abort "only $UPLOADS_OK of 3 neuron uploads succeeded"
fi

if ./scripts/governance_calculate_voting_powers.sh >/dev/null 2>&1; then
  pass "calculated voting powers"
else
  abort "calculate_voting_powers failed"
fi

# With the layers from governance_deploy.sh (all weights 1.0, layer 0 sums
# Neuron1 and Neuron2, layer 1 is Neuron3 alone) the power is N1 + N2 + N3.
# bc because a sum of i64 values can overflow shell arithmetic.

step "5. Check voting powers read from NQG"

expected_power() {
  local id=$1
  local n1 n2 n3
  n1=$(jq -r ".Neuron1[\"$id\"]" "$NEURONS_FILE")
  n2=$(jq -r ".Neuron2[\"$id\"]" "$NEURONS_FILE")
  n3=$(jq -r ".Neuron3[\"$id\"]" "$NEURONS_FILE")
  echo "$n1 + $n2 + $n3" | bc
}

for id in $VOTERS; do
  EXPECTED=$(expected_power "$id")
  ACTUAL=$(read_only "$NQG" get_voting_power_for_user --member_id "$id" | unquote)
  if [ "$ACTUAL" = "$EXPECTED" ]; then
    pass "member $id has voting power $ACTUAL"
  else
    fail "member $id: expected $EXPECTED, NQG returned '$ACTUAL'"
  fi
done

# governance(token_id).nqg has 6 decimals, like NQG. A membership build that
# does not read NQG's i64 by token id shows 0: a warning, not a failure.

step "6. Connect membership to NQG and read scores through it"

if send "$KEY_PREFIX-mem-admin" "$MEMBERSHIP" set_nqg_contract --nqg_contract "$NQG" >/dev/null 2>&1; then
  pass "membership contract now points at NQG"
else
  fail "set_nqg_contract failed"
fi

MEMBERSHIP_UPDATED=true
for id in $VOTERS; do
  EXPECTED=$(expected_power "$id")
  ACTUAL=$(read_only "$MEMBERSHIP" governance --token_id "$id" | jq -r '.nqg')
  if [ "$ACTUAL" = "$EXPECTED" ]; then
    pass "membership shows member $id with NQG score $ACTUAL"
  elif [ "$ACTUAL" = "0" ]; then
    MEMBERSHIP_UPDATED=false
    echo "WARN: membership shows 0 for member $id (expected $EXPECTED)"
  else
    fail "membership shows member $id with '$ACTUAL', expected $EXPECTED"
  fi
done
if [ "$MEMBERSHIP_UPDATED" = false ]; then
  echo "WARN: scores read as 0 through the membership contract. Its get_nqg must"
  echo "      call get_voting_power_for_user with the token id and decode an i64; see"
  echo "      contracts/governance/README.md, 'Connecting to Stellar Membership'."
fi

step "7. Revoke member $REVOKED_ID and check NQG refuses it"

# --revoked is a bool flag: without it the call reinstates, a silent no-op.
send "$KEY_PREFIX-m$REVOKED_ID" "$MEMBERSHIP" revoke \
  --operator "$KEY_PREFIX-m$REVOKED_ID" --token_id "$REVOKED_ID" --revoked >/dev/null 2>&1
STATUS=$(read_only "$MEMBERSHIP" member --token_id "$REVOKED_ID" | jq -r '.status')
if [ "$STATUS" = "1" ]; then
  pass "revoked member $REVOKED_ID"
else
  abort "member $REVOKED_ID still has status '$STATUS' after revoke"
fi

if ./scripts/membership_check_voters.sh >/dev/null 2>&1; then
  fail "membership_check_voters.sh did not notice the revoked member"
else
  pass "membership_check_voters.sh reports the revoked member"
fi

UPLOAD_OUTPUT=$(./scripts/governance_upload_neurons_results.sh 2>&1)
# Error lines only: the CLI repeats the code in a diagnostic event.
REJECTED=$(echo "$UPLOAD_OUTPUT" | grep "error:" | grep -c "Error(Contract, #18)")
if [ "$REJECTED" -eq 3 ]; then
  pass "all 3 uploads rejected with NotAMember (#18)"
else
  fail "expected 3 uploads rejected with #18, got $REJECTED"
fi

step "Summary"
echo "Membership contract: $MEMBERSHIP"
echo "NQG contract:        $NQG"
echo "Config used:         $ENV_PATH"
echo "Passed: $PASSED  Failed: $FAILED"
[ "$FAILED" -eq 0 ]
