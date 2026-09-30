# How to run Neural Quorum Governance with Stellar Membership

This guide shows how to set up and run the Neural Quorum Governance (NQG) contract end to end. It uses the actual Soroban smart contract, but everything else is a minimal working setup, just to demonstrate the flow. For this tutorial we use json files instead of a database. Example files with data are located in the `data` folder.

Voters are identified by their Stellar Membership token id: a `u32` assigned sequentially at mint (`0` for the first member, `1` for the second, and so on). The id stays the same when a member rotates or recovers their key, so their NQG score follows them. The NQG contract only accepts neuron results for ids that are active members of the membership contract.

The NQG contract only computes voting powers (NQG scores). It does not collect or count votes: other contracts and apps read the scores from it.

`cd examples` to make sure all scripts work correctly. The scripts need the [stellar CLI](https://developers.stellar.org/docs/tools/cli) and `jq`.

## Voters
`data/voters.json` lists the voters of the round as membership token ids:

```json
[0, 1, 2, 3, 4]
```

Every id must be an active member (minted and not revoked) of the membership contract you point the NQG contract at. Adjust the list to the members that exist on your membership contract.

## Neurons
Neurons are used to calculate component values of voting power. We input some data into each neuron, and it outputs a numeric value. Output values are converted to fixed point integers with 6 decimals (1.0 is `1000000`), the `i64` format the contract stores, so no precision is lost between formats. Then the results of all neurons have to be uploaded to the NQG contract, which will use this data to calculate the final voting power.

Why not upload all data into the contract and calculate all values there?
Doing so would be beneficial for transparency of the whole system, but comes with 2 problems:
 - privacy - some of the data we use for neurons would allow bad actors to learn more about specific members than they chose to publish.
 - performance - due to high amount of data, and high complexity of calculations doing everything on-chain would not be possible in a reasonable time and cost.

### Minimal neurons setup
In `/neurons` you can see a rust project that contains example neurons, along with the code that will trigger them. In the `data` folder there is some example input for each neuron, keyed by token id. Neurons can perform any type of calculations, for example provide 0.5 points bonus for each round a member has participated in, or something more complicated. In this example we have 3 neurons, called Neuron1 Neuron2 and Neuron3.

Neuron1: Multiplies input value by 1.5
Neuron2: Subtracts 20% from the input value
Neuron3: Multiplies input value by 3

If you have rust installed, head into the correct folder and run:

`pushd neurons`
`cargo run`
`popd`

In the `data` folder you'll see `neurons_output.json`, one map per neuron from token id to result:

```json
{
  "Neuron1": {
    "0": 15000000,
    "1": 30000000
  }
}
```

JSON object keys are always strings, so ids are written as `"0"`, `"1"`, ...; the stellar CLI parses them back into the contract's `u32` keys. Values are plain JSON numbers. Now that we have the neurons results we can move on to the on-chain part of the system.

## NQG Contract
The NQG contract is used to calculate voting powers of members based on the supplied neurons results. Source code for it is located in `/contracts/governance`.

### Configuring
In the `examples` folder create a `.env` file, by changing the name of `env.example` and filling in your account data and `MEMBERSHIP_CONTRACT_ADDRESS`, the Stellar Membership contract whose members vote.

### Checking the voters
Before uploading anything, check that every id in `data/voters.json` is an active member:

`./scripts/membership_check_voters.sh`

It reads `member(token_id).status` on the membership contract for each id and exits with an error if any id was never minted or is revoked.

### Deploying and initializing the contract
This script will compile, deploy and initialize the NQG contract with your account as the admin, the round from `.env` and the membership contract. It also sets up the layers (more about layers see `Uploading neurons results`). After deployment the address of the contract will be saved in the .env file for future use.

`./scripts/governance_deploy.sh`

### Uploading neurons results
Neurons results need to be uploaded to allow calculating the final NQG score (voting power) on-chain. Each neuron is assigned to a "layer" and has its own id. For example we have 3 neurons, each assigned to a specific neuron in the contract.
Layer 0: neuron 0 and 1
Layer 1: neuron 0
Each Layer calculates its output based on how it was configured during deployment. In our examples we configured the first layer as "Sum" - it means values from all neurons on this layer will be summed together, and the second layer as "Product" - values from neurons will be multiplied, although we have only one neuron on that layer so its output will be equal to the neuron value.
Results of each neuron, calculated beforehand, will now be uploaded to the contract to its corresponding layer and neuron id. If any token id in a result is not an active member, the contract rejects the whole upload with `NotAMember`.

`./scripts/governance_upload_neurons_results.sh`

### Calculating voting powers
After uploading neurons, we can trigger the calculation of voting powers. This function will run all of the Layers and save the results in storage, so they can be accessed anytime.

`./scripts/governance_calculate_voting_powers.sh`

### Reading voting powers
The scores for the current round can be read with `get_voting_powers` (all members) or `get_voting_power_for_user` (one member, by token id). This is also how other contracts, such as the membership contract, read a member's NQG score.

`./scripts/governance_get_voting_powers.sh`

### Setting the round number
After a round is complete, it is possible to change the round number on the contract, and use it for the next round, without impacting previously saved data. This script will set the current round on the contract to the one specified in the .env file, and fetch the current round from the contract to verify it was correctly set.

`./scripts/governance_set_round.sh`

## What's next
Now you have a working bare-minimum setup of Neural Quorum Governance. You can create a backend service that connects all of those elements into one api, however it suits your project, using one of many stellar sdk's.
