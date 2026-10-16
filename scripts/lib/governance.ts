import { GUESTS, type Neuron } from "./neurons.ts";
import type { ProofOutput } from "./proof.ts";
import { read, send, type Target } from "./stellar.ts";

/** A neuron's place in the governance contract. */
export type Slot = { layerId: string; neuronId: string };

function slotArgs(slot: Slot): string[] {
  return ["--layer_id", slot.layerId, "--neuron_id", slot.neuronId];
}

/** The round the contract stores results under. */
export function activeRound(governance: Target): string {
  return String(read(governance, "get_current_round"));
}

/** The name the neuron was added with. Fails if the contract has no such neuron. */
export function neuronName(governance: Target, slot: Slot): string {
  const neuron = read(governance, "get_neuron", slotArgs(slot)) as { name: string };
  return neuron.name;
}

/** Every layer and neuron the contract has, in order. */
export function layout(governance: Target): Slot[] {
  const { layers } = read(governance, "get_neural_governance") as { layers: string[] };
  return layers.flatMap((layerId) => {
    const { neurons } = read(governance, "get_layer", ["--layer_id", layerId]) as {
      neurons: string[];
    };
    return neurons.map((neuronId) => ({ layerId, neuronId }));
  });
}

/** Send the scores and their proof. The contract stores them under its active round. */
export function setNeuronResult(
  governance: Target,
  slot: Slot,
  neuron: Neuron,
  output: ProofOutput,
): void {
  send(governance, "set_neuron_result", [
    ...slotArgs(slot),
    "--result", output.result,
    "--guest", GUESTS[neuron],
    "--journal", output.journal,
    "--seal", output.seal,
  ]);
}
