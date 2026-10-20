import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { loadConfig, target } from "./lib/config.ts";
import { readMembers, type Member } from "./lib/member.ts";
import { read } from "./lib/stellar.ts";
import { runPaths, writeJson, type Snapshot } from "./lib/run.ts";
import { readVotingHistory, type VotingHistory } from "./lib/voting-history.ts";

const USAGE = `Usage:
  node scripts/collect.ts [--config <flow.config.json>] [--out <run dir>]

Reads active members and project voting history, adds the mocked Discord
roles and trust graph from scripts/mocks/, and writes a run folder with the
input of every neuron. Prints the run folder.`;

const MOCKS = fileURLToPath(new URL("mocks/", import.meta.url));
const RUNS = fileURLToPath(new URL("../runs/", import.meta.url));

/** Prior voting history refuses lower rounds (`validate_current_round`). */
const MIN_ROUND = 8;

/** Mocked data, keyed by membership token id. */
type Mocks = {
  /** id -> Discord role names (`scripts/mocks/discord-roles.json`). */
  discordRoles: Record<string, string[]>;
  /** id -> ids it trusts (`scripts/mocks/trust-graph.json`). */
  trustedForUser: Record<string, string[]>;
};

function parseFlags() {
  try {
    return parseArgs({
      options: { config: { type: "string" }, out: { type: "string" } },
      strict: true,
    }).values;
  } catch (error) {
    throw new Error(`${error instanceof Error ? error.message : error}\n${USAGE}`);
  }
}

function readMock(name: string): unknown {
  const path = `${MOCKS}${name}`;
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch (error) {
    throw new Error(`cannot read mock ${path}: ${error instanceof Error ? error.message : error}`);
  }
}

function isIdList(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((id) => typeof id === "string");
}

function readMocks(): Mocks {
  const discordRoles = readMock("discord-roles.json") as Mocks["discordRoles"];
  const trustedForUser = readMock("trust-graph.json") as Mocks["trustedForUser"];

  for (const [id, roles] of Object.entries(discordRoles)) {
    if (!isIdList(roles)) throw new Error(`discord-roles.json: ${id} must map to a list of names`);
  }
  for (const [id, trusted] of Object.entries(trustedForUser)) {
    if (!isIdList(trusted)) throw new Error(`trust-graph.json: ${id} must map to a list of ids`);
    if (trusted.includes(id)) throw new Error(`trust-graph.json: ${id} trusts itself`);
  }
  return { discordRoles, trustedForUser };
}

/**
 * Keeps ids of active members; collects the others so they can be reported
 * once per data source. Revoked members do not count, not even as someone's
 * trusted id or as a voter.
 */
function activeFilter(users: string[]) {
  const active = new Set(users);
  const dropped = new Set<string>();
  return {
    keep(id: string): boolean {
      if (active.has(id)) return true;
      dropped.add(id);
      return false;
    },
    report(file: string) {
      if (dropped.size) {
        const ids = [...dropped].sort((a, b) => Number(a) - Number(b));
        console.error(`${file}: dropped ids that are not active members: ${ids.join(", ")}`);
      }
      dropped.clear();
    },
  };
}

function buildInputs(active: Member[], mocks: Mocks, history: VotingHistory) {
  const users = active.map((member) => String(member.id));
  const { keep, report } = activeFilter(users);

  for (const id of Object.keys(mocks.discordRoles)) keep(id);
  report("discord-roles.json");
  const assignedReputation = {
    users: active.map((member) => ({
      id: String(member.id),
      tier: member.role,
      discord_roles: mocks.discordRoles[String(member.id)] ?? [],
    })),
  };

  const trustedForUser: Record<string, string[]> = {};
  for (const [id, trusted] of Object.entries(mocks.trustedForUser)) {
    if (!keep(id)) continue;
    const kept = trusted.filter(keep);
    if (kept.length) trustedForUser[id] = kept;
  }
  report("trust-graph.json");
  if (Object.keys(trustedForUser).length === 0) {
    // The trust graph guest panics on an empty graph.
    throw new Error("trust-graph.json has no trust between active members");
  }
  const trustGraph = { users, trustedForUser };

  for (const id of history.users) keep(id);
  report("voting history");
  const priorVotingHistory = {
    users,
    usersRoundHistory: Object.fromEntries(users.map(id => [id, history.usersRoundHistory[id] ?? []])),
  };

  return {
    "prior-voting-history": priorVotingHistory,
    "assigned-reputation": assignedReputation,
    "trust-graph": trustGraph,
  } as const;
}

async function main() {
  const flags = parseFlags();
  const config = loadConfig(flags.config);
  const governance = target(config, config.governance);

  const round = Number(read(governance, "get_current_round"));
  if (round < MIN_ROUND) {
    throw new Error(`the active round is ${round}; prior voting history needs at least ${MIN_ROUND}`);
  }
  const membership = String(read(governance, "get_membership_contract"));
  const { members, ledger } = await readMembers(config, membership);
  const active = members.filter((member) => !member.revoked);
  if (active.length === 0) throw new Error("the membership contract has no active members");
  console.error(
    `Round ${round}: ${active.length} active members` +
      (members.length > active.length ? `, ${members.length - active.length} revoked` : ""),
  );

  const mocks = readMocks();
  const history = await readVotingHistory(target(config, membership), config.votingHistory);
  const inputs = buildInputs(active, mocks, history);
  const createdAt = new Date().toISOString();
  const dir = resolve(flags.out ?? `${RUNS}r${round}-${createdAt.replace(/[:.]/g, "-")}`);
  if (existsSync(dir)) throw new Error(`${dir} already exists`);
  const paths = runPaths(dir);
  mkdirSync(paths.inputs, { recursive: true });

  const snapshot: Snapshot = {
    createdAt,
    rpcUrl: config.rpcUrl,
    networkPassphrase: config.networkPassphrase,
    governance: config.governance,
    membership,
    round,
    ledger,
    members,
    users: active.map((member) => String(member.id)),
    votingHistoryRounds: history.roundByProject,
    mocked: [
      "assigned-reputation: discord_roles",
      "trust-graph: trustedForUser",
    ],
  };
  writeJson(paths.snapshot, snapshot);
  for (const [neuron, input] of Object.entries(inputs)) {
    writeJson(paths.input(neuron as keyof typeof inputs), input);
  }
  console.error(`Saved the inputs of round ${round}.`);
  console.log(dir);
}

try {
  await main();
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
}
