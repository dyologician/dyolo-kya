/**
 * a1 swarm — SwarmPassport TypeScript client.
 *
 * ```typescript
 * import { SwarmClient, SwarmRole } from '@a1/sdk/swarm';
 *
 * const swarm = new SwarmClient('http://localhost:8080', { adminSecret: process.env.A1_ADMIN_SECRET });
 *
 * const swarmId = await swarm.createSwarm({
 *   name: 'acme-trading-swarm',
 *   capabilities: ['trade.equity', 'portfolio.read'],
 *   ttlDays: 30,
 *   signingKeyHex: ROOT_SK_HEX,
 * });
 *
 * const member = await swarm.addMember({
 *   swarmId,
 *   agentPkHex: WORKER_PK,
 *   role: SwarmRole.Worker,
 *   capabilities: ['trade.equity'],
 *   ttlSeconds: 3600,
 *   signingKeyHex: ROOT_SK_HEX,
 * });
 * ```
 */

export enum SwarmRole {
  Orchestrator = 'orchestrator',
  Worker = 'worker',
  Supervisor = 'supervisor',
  Auditor = 'auditor',
}

export interface SwarmMemberInfo {
  agentDid: string;
  agentPkHex: string;
  role: string;
  issuedAtUnix: number;
  expiresAtUnix: number;
  certFingerprintHex?: string;
}

export interface CreateSwarmOptions {
  name: string;
  capabilities: string[];
  ttlDays?: number;
  signingKeyHex: string;
}

export interface AddMemberOptions {
  swarmId: string;
  agentPkHex: string;
  role: SwarmRole | string;
  capabilities: string[];
  ttlSeconds?: number;
  signingKeyHex: string;
}

export interface GetChainOptions {
  swarmId: string;
  agentPkHex: string;
  ttlSeconds?: number;
  signingKeyHex: string;
}

export class SwarmError extends Error {
  constructor(message: string, public readonly code = 'SWARM_ERROR') {
    super(message);
    this.name = 'SwarmError';
  }
}

export class SwarmClient {
  private readonly base: string;
  private readonly headers: Record<string, string>;

  constructor(gatewayUrl: string, opts: { adminSecret?: string } = {}) {
    this.base = gatewayUrl.replace(/\/$/, '');
    this.headers = {
      'Content-Type': 'application/json',
      ...(opts.adminSecret ? { Authorization: `Bearer ${opts.adminSecret}` } : {}),
    };
  }

  async createSwarm(opts: CreateSwarmOptions): Promise<string> {
    const data = await this.post('/v1/swarm/create', {
      swarm_name: opts.name,
      capabilities: opts.capabilities,
      ttl_days: opts.ttlDays ?? 30,
      signing_key_hex: opts.signingKeyHex,
    });
    return data.swarm_id as string;
  }

  async addMember(opts: AddMemberOptions): Promise<SwarmMemberInfo> {
    const data = await this.post('/v1/swarm/member/add', {
      swarm_id: opts.swarmId,
      agent_pk_hex: opts.agentPkHex,
      role: typeof opts.role === 'string' ? opts.role : opts.role,
      capabilities: opts.capabilities,
      ttl_seconds: opts.ttlSeconds ?? 3600,
      signing_key_hex: opts.signingKeyHex,
    });
    const m = data.member as Record<string, unknown>;
    return {
      agentDid: m.agent_did as string,
      agentPkHex: m.agent_pk_hex as string,
      role: m.role as string,
      issuedAtUnix: m.issued_at_unix as number,
      expiresAtUnix: m.expires_at_unix as number,
      certFingerprintHex: m.cert_fingerprint_hex as string | undefined,
    };
  }

  async getChain(opts: GetChainOptions): Promise<Record<string, unknown>> {
    return this.post('/v1/swarm/member/chain', {
      swarm_id: opts.swarmId,
      agent_pk_hex: opts.agentPkHex,
      ttl_seconds: opts.ttlSeconds ?? 3600,
      signing_key_hex: opts.signingKeyHex,
    });
  }

  async listMembers(swarmId: string): Promise<SwarmMemberInfo[]> {
    const data = await this.get(`/v1/swarm/${swarmId}/members`);
    return ((data.members ?? []) as Record<string, unknown>[]).map(m => ({
      agentDid: m.agent_did as string,
      agentPkHex: m.agent_pk_hex as string,
      role: m.role as string,
      issuedAtUnix: m.issued_at_unix as number,
      expiresAtUnix: m.expires_at_unix as number,
      certFingerprintHex: m.cert_fingerprint_hex as string | undefined,
    }));
  }

  async removeMember(swarmId: string, agentDid: string): Promise<void> {
    await this.post('/v1/swarm/member/remove', { swarm_id: swarmId, agent_did: agentDid });
  }

  private async post(path: string, body: unknown): Promise<Record<string, unknown>> {
    const resp = await fetch(`${this.base}${path}`, {
      method: 'POST', headers: this.headers, body: JSON.stringify(body),
    });
    if (!resp.ok) {
      const err = await resp.json().catch(() => ({ error: resp.statusText }));
      throw new SwarmError((err as Record<string, string>).error ?? resp.statusText);
    }
    return resp.json();
  }

  private async get(path: string): Promise<Record<string, unknown>> {
    const resp = await fetch(`${this.base}${path}`, { headers: this.headers });
    if (!resp.ok) {
      const err = await resp.json().catch(() => ({ error: resp.statusText }));
      throw new SwarmError((err as Record<string, string>).error ?? resp.statusText);
    }
    return resp.json();
  }
}