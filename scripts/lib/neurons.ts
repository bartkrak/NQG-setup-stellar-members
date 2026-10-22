/** The neurons of the flow, by their `host prove --neuron` name, in proving order. */
export const NEURONS = ["prior-voting-history", "assigned-reputation", "trust-graph", "trust-loss"] as const;

export type Neuron = (typeof NEURONS)[number];

/** `host prove --neuron` name to the `guest` the governance contract expects. */
export const GUESTS: Record<Neuron, string> = {
  "prior-voting-history": "PriorVotingHistory",
  "assigned-reputation": "AssignedReputation",
  "trust-graph": "TrustGraph",
  "trust-loss": "TrustLoss",
};

export function isNeuron(name: string): name is Neuron {
  return (NEURONS as readonly string[]).includes(name);
}
