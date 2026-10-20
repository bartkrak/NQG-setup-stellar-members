import { scValToNative, xdr } from "@stellar/stellar-sdk";
import type { VotingHistoryConfig } from "./config.ts";
import { readMemberAddresses, resolveMemberId } from "./member.ts";
import { isContractError, read, stellar, type Target } from "./stellar.ts";

const NO_PROPOSAL_OR_PAGE_FOUND = 301;

export type VotingHistory = {
  users: string[];
  usersRoundHistory: Record<string, number[]>;
  roundByProject: Record<string, number>;
};

type Proposal = {
  id: number;
  proposer: string;
  vote_data: { votes: Record<string, { address: string; weight: number }>[] };
};

function proposalIdLimit(tansu: Target, projectKey: string): number {
  const key = xdr.ScVal.scvVec([
    xdr.ScVal.scvSymbol("DaoTotalProposals"),
    xdr.ScVal.scvBytes(Buffer.from(projectKey, "hex")),
  ]).toXdr("base64");
  const output = stellar([
    "contract", "read", ...tansu.network, "--id", tansu.contract,
    "--key-xdr", key, "--output", "xdr",
  ]);
  if (output === "") {
    const project = read(tansu, "get_project", ["--project_key", projectKey]) as { name: string };
    if (typeof project?.name !== "string") throw new Error("Invalid project response.");
    return 0;
  }
  const fields = output.split(",");
  if (fields.length !== 4 || fields[0] !== key) {
    throw new Error("Unexpected proposal counter response from stellar contract read.");
  }
  const value = xdr.ScVal.fromXdr(fields[1], "base64");
  if (value.type !== "scvU32") throw new Error("Proposal counter is not a u32.");
  return value.u32;
}

function readProposalVoters(tansu: Target, projectKey: string, id: number): string[] | null {
  let proposal: Proposal;
  try {
    proposal = read(tansu, "get_proposal", [
      "--project_key", projectKey, "--proposal_id", String(id),
    ]) as Proposal;
  } catch (error) {
    if (isContractError(error, NO_PROPOSAL_OR_PAGE_FOUND)) {
      return null;
    }
    throw error;
  }
  if (proposal?.id !== id || typeof proposal.proposer !== "string" ||
      !Array.isArray(proposal.vote_data?.votes)) {
    throw new Error(`get_proposal ${id} returned an invalid proposal.`);
  }
  const votes: string[] = [];
  for (const vote of proposal.vote_data.votes) {
    const body = vote?.PublicVote ?? vote?.AnonymousVote;
    if (!body || Object.keys(vote).length !== 1 || typeof body.address !== "string") {
      throw new Error(`Proposal ${id} contains an invalid vote record.`);
    }
    if (body.address === proposal.proposer && body.weight === 0) continue;
    votes.push(body.address);
  }
  return votes;
}

function subProjectKeys(tansu: Target, parentProjectKey: string): string[] {
  const keys = read(tansu, "get_sub_projects", [
    "--project_key", parentProjectKey,
  ]) as string[];
  return [...new Set(keys.map((key) => key.toLowerCase()))];
}

/** Read one page of project keys in original registration order. */
async function readRegistrationPage(config: VotingHistoryConfig, cursor: string): Promise<{ projectKeys: string[]; nextCursor: string }> {
  const url = new URL(`${config.explorerUrl}/contract/${config.tansuContract}/events`);
  url.search = new URLSearchParams({ topic: "project_registered", order: "asc", limit: "200" }).toString();

  if (cursor) url.searchParams.set("cursor", cursor);

  const response = await fetch(url, { signal: AbortSignal.timeout(30_000) });
  if (!response.ok) throw new Error(`Cannot read project registrations: HTTP ${response.status}`);

  const events = (await response.json())?._embedded?.records;
  if (!Array.isArray(events)) throw new Error("Invalid project registration response.");

  const nextCursor = events.at(-1)?.paging_token ?? "";
  if (events.length && (typeof nextCursor !== "string" || !nextCursor)) {
    throw new Error("Registration history pagination did not advance.");
  }

  const projectKeys = events.map(event => {
    const key = scValToNative(xdr.ScVal.fromXdr(event.topicsXdr[1], "base64"));
    return Buffer.from(key).toString("hex");
  });
  return { projectKeys, nextCursor };
}

/** Number requested projects from 1, oldest registration first. */
async function projectRounds(config: VotingHistoryConfig, projectKeys: string[]): Promise<Record<string, number>> {
  const remainingProjects = new Set(projectKeys);
  const roundByProject = new Map<string, number>();
  const seenCursors = new Set<string>();
  let cursor = "";
  while (remainingProjects.size) {
    const page = await readRegistrationPage(config, cursor);
    if (!page.projectKeys.length) {
      throw new Error(`No original registration found for ${[...remainingProjects].join(", ")}.`);
    }
    if (seenCursors.has(page.nextCursor)) {
      throw new Error("Registration history pagination did not advance.");
    }
    seenCursors.add(page.nextCursor);
    for (const projectKey of page.projectKeys) {
      if (!remainingProjects.has(projectKey)) continue;
      roundByProject.set(projectKey, roundByProject.size + 1);
      remainingProjects.delete(projectKey);
    }
    cursor = page.nextCursor;
  }
  return Object.fromEntries(roundByProject);
}

/** Read project participation and map addresses to ids in the supplied membership contract. */
export async function readVotingHistory(membership: Target, config: VotingHistoryConfig): Promise<VotingHistory> {
  const tansu = { ...membership, contract: config.tansuContract };
  const projectKeys = subProjectKeys(tansu, config.parentProjectKey);
  if (!projectKeys.length) return { users: [], usersRoundHistory: {}, roundByProject: {} };
  const roundByProject = await projectRounds(config, projectKeys);
  const history: Record<string, Set<number>> = Object.create(null);
  for (const projectKey of projectKeys) {
    const limit = proposalIdLimit(tansu, projectKey);
    for (let id = 0; id < limit; id++) {
      const voters = readProposalVoters(tansu, projectKey, id);
      if (!voters) continue;
      for (const address of voters) {
        const rounds = history[address] ??= new Set<number>();
        rounds.add(roundByProject[projectKey]);
      }
    }
  }

  const voters = Object.entries(history);
  if (!voters.length) return { users: [], usersRoundHistory: {}, roundByProject };
  const memberIdByAddress = await readMemberAddresses(membership.contract, config.explorerUrl);
  const memberHistory: Record<string, Set<number>> = Object.create(null);
  for (const [address, rounds] of voters) {
    const id = resolveMemberId(membership, address, memberIdByAddress);
    if (id === null) continue;
    const userRounds = memberHistory[id] ??= new Set<number>();
    for (const round of rounds) userRounds.add(round);
  }

  const users = Object.keys(memberHistory).sort((a, b) => Number(a) - Number(b));
  return {
    users,
    usersRoundHistory: Object.fromEntries(users.map(id => [id, [...memberHistory[id]].sort((a, b) => a - b)])),
    roundByProject,
  };
}
