import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { Address, rpc, scValToNative, xdr } from "@stellar/stellar-sdk";
import { loadConfig, target, type FlowConfig } from "./lib/config.ts";
import { read } from "./lib/stellar.ts";
import { runPaths, writeJson, type Member, type Snapshot } from "./lib/run.ts";

const USAGE = `Usage:
  node scripts/collect.ts [--config <flow.config.json>] [--out <run dir>]

Reads the active members from the membership contract the governance contract
uses, merges the mocked data from scripts/mocks/, and writes a run folder with
the input of every neuron. Prints the run folder.`;

const MOCKS = fileURLToPath(new URL("mocks/", import.meta.url));
const RUNS = fileURLToPath(new URL("../runs/", import.meta.url));

/** Most ledger keys one `getLedgerEntries` request takes. */
const LEDGER_KEYS_PER_REQUEST = 200;
/** Prior voting history refuses lower rounds (`validate_current_round`). */
const MIN_ROUND = 8;
const VOTES = new Set(["Yes", "No", "Delegate", "Abstain"]);

/** Mocked data, keyed by membership token id. */
type Mocks = {
  /** id -> Discord role names (`scripts/mocks/discord-roles.json`). */
  discordRoles: Record<string, string[]>;
  /** id -> ids it trusts (`scripts/mocks/trust-graph.json`). */
  trustedForUser: Record<string, string[]>;
  /** The prior voting history input minus `users` (`scripts/mocks/prior-voting-history.json`). */
  votingHistory: {
    usersRoundHistory: Record<string, number[]>;
    votesPerRound: Record<string, Record<string, Record<string, string>>>;
    submittersPerRound: Record<string, string[]>;
  };
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

function memberKey(membership: string, tokenId: number): xdr.LedgerKey {
  return xdr.LedgerKey.contractData(new xdr.LedgerKeyContractData({
    contract: new Address(membership).toScAddress(),
    key: xdr.ScVal.scvVec([
      xdr.ScVal.scvSymbol("Member"),
      xdr.ScVal.scvU32(tokenId),
    ]),
    durability: xdr.ContractDataDurability.persistent,
  }));
}

/**
 * Every minted token, read straight from the membership contract's storage
 * (`MemberKey::Member(token_id)`), up to 200 per request.
 */
async function readMembers(
  config: FlowConfig,
  membership: string,
): Promise<{ members: Member[]; ledger: number }> {
  const count = Number(read(target(config, membership), "next_token_id"));
  const server = new rpc.Server(config.rpcUrl, {
    timeout: 30_000,
    // A local network serves plain HTTP.
    allowHttp: config.rpcUrl.startsWith("http://"),
  });
  const found = new Map<number, Member>();
  let ledger = 0;
  console.error(`Reading ${count} members of ${membership}...`);
  for (let start = 0; start < count; start += LEDGER_KEYS_PER_REQUEST) {
    const keys: xdr.LedgerKey[] = [];
    for (let id = start; id < Math.min(count, start + LEDGER_KEYS_PER_REQUEST); id++) {
      keys.push(memberKey(membership, id));
    }
    const response = await server.getLedgerEntries(...keys);
    ledger = response.latestLedger;
    for (const entry of response.entries) {
      if (entry.val.type !== "contractData") {
        throw new Error("Unexpected ledger entry type");
      }
      const data = entry.val.contractData;
      const [, id] = scValToNative(data.key) as [string, number];
      const { status, role } = scValToNative(data.val) as { status: number; role: number };
      found.set(id, { id, status, role });
    }
  }
  const members: Member[] = [];
  for (let id = 0; id < count; id++) {
    const member = found.get(id);
    if (!member) {
      throw new Error(`member ${id} is not in live storage (archived?)`);
    }
    members.push(member);
  }
  return { members, ledger };
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
  const votingHistory = readMock("prior-voting-history.json") as Mocks["votingHistory"];

  for (const [id, roles] of Object.entries(discordRoles)) {
    if (!isIdList(roles)) throw new Error(`discord-roles.json: ${id} must map to a list of names`);
  }
  for (const [id, trusted] of Object.entries(trustedForUser)) {
    if (!isIdList(trusted)) throw new Error(`trust-graph.json: ${id} must map to a list of ids`);
    if (trusted.includes(id)) throw new Error(`trust-graph.json: ${id} trusts itself`);
  }
  const { usersRoundHistory, votesPerRound, submittersPerRound } = votingHistory;
  if (!usersRoundHistory || !votesPerRound || !submittersPerRound) {
    throw new Error(
      "prior-voting-history.json needs usersRoundHistory, votesPerRound and submittersPerRound",
    );
  }
  for (const [round, submissions] of Object.entries(votesPerRound)) {
    for (const votes of Object.values(submissions)) {
      for (const [id, vote] of Object.entries(votes)) {
        if (!VOTES.has(vote)) {
          throw new Error(`prior-voting-history.json: vote of ${id} in round ${round} is ${vote}`);
        }
      }
    }
  }
  return { discordRoles, trustedForUser, votingHistory };
}

/**
 * Keeps ids of active members; collects the others so they can be reported
 * once per mock file. Revoked members do not count, not even as someone's
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

function filterKeys<T>(record: Record<string, T>, keep: (id: string) => boolean) {
  return Object.fromEntries(Object.entries(record).filter(([id]) => keep(id)));
}

function buildInputs(active: Member[], mocks: Mocks) {
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

  const history = mocks.votingHistory;
  const priorVotingHistory = {
    users,
    usersRoundHistory: filterKeys(history.usersRoundHistory, keep),
    votesPerRound: Object.fromEntries(
      Object.entries(history.votesPerRound).map(([round, submissions]) => [
        round,
        Object.fromEntries(
          Object.entries(submissions).map(([project, votes]) => [project, filterKeys(votes, keep)]),
        ),
      ]),
    ),
    submittersPerRound: Object.fromEntries(
      Object.entries(history.submittersPerRound).map(([round, ids]) => [round, ids.filter(keep)]),
    ),
  };
  report("prior-voting-history.json");

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
  const active = members.filter((member) => member.status === 0);
  if (active.length === 0) throw new Error("the membership contract has no active members");
  console.error(
    `Round ${round}: ${active.length} active members` +
      (members.length > active.length ? `, ${members.length - active.length} revoked` : ""),
  );

  const inputs = buildInputs(active, readMocks());
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
    mocked: [
      "assigned-reputation: discord_roles",
      "trust-graph: trustedForUser",
      "prior-voting-history: usersRoundHistory, votesPerRound, submittersPerRound",
    ],
  };
  writeJson(paths.snapshot, snapshot);
  for (const [neuron, input] of Object.entries(inputs)) {
    writeJson(paths.input(neuron as keyof typeof inputs), input);
  }
  console.error(`Saved the inputs of round ${round}. Next: node scripts/prove-all.ts --run <dir>`);
  console.log(dir);
}

try {
  await main();
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
}
