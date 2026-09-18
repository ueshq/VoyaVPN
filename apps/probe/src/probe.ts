/**
 * `POST /v1/probe`: connect back to the caller's own address on up to four
 * ports and report which ones accepted a TCP connection.
 *
 * The request names ports only. The address is always the one Cloudflare saw
 * the request come from, so the service cannot be pointed at anyone else and
 * is useless as a scanner. Nothing is stored; the rate limit is per caller.
 *
 * The wire shapes are pinned by `tests/probe-contract/*.json`, which the Rust
 * client in `crates/voya-net/src/probe/reachability.rs` reads as well.
 */

import { addressFamily, isPublicAddress, type AddressFamily } from "./address";

const PROBE_PATH = "/v1/probe";
const MAX_PORTS = 4;
/** Below this, ports belong to system services the node never uses. */
const MIN_PORT = 1024;
const CONNECT_TIMEOUT_MS = 4000;
const MAX_BODY_BYTES = 256;

export type ProbeOutcome = "connected" | "timeout" | "refused" | "error";
type ErrorCode = "rateLimited" | "invalid" | "probeDenied" | "notFound" | "methodNotAllowed";

type PortResult = {
  port: number;
  reachable: boolean;
  reason: ProbeOutcome;
  elapsedMs: number;
};

type ProbeResponse = {
  ip: string;
  family: AddressFamily;
  results: PortResult[];
};

/** Opens a TCP connection and reports how it ended; never throws. */
export type Dialer = (address: string, port: number, timeoutMs: number) => Promise<ProbeOutcome>;

export type ProbeDeps = {
  dial: Dialer;
  /** Per-caller rate limit; absent in local development. */
  limiter?: { limit(options: { key: string }): Promise<{ success: boolean }> };
  now?: () => number;
};

export async function handleRequest(request: Request, deps: ProbeDeps): Promise<Response> {
  const url = new URL(request.url);
  if (url.pathname !== PROBE_PATH) return errorResponse(404, "notFound");
  if (request.method !== "POST") return errorResponse(405, "methodNotAllowed");

  const ip = request.headers.get("CF-Connecting-IP")?.trim() ?? "";
  const family = addressFamily(ip);
  if (!family || !isPublicAddress(ip)) return errorResponse(403, "probeDenied");
  if (deps.limiter && !(await deps.limiter.limit({ key: ip })).success) {
    return errorResponse(429, "rateLimited");
  }

  const ports = parsePorts(await readBody(request));
  if (!ports) return errorResponse(400, "invalid");

  const now = deps.now ?? Date.now;
  const results = await Promise.all(
    ports.map(async (port): Promise<PortResult> => {
      const started = now();
      const reason = await deps.dial(ip, port, CONNECT_TIMEOUT_MS);
      return {
        port,
        reachable: reason === "connected",
        reason,
        elapsedMs: Math.max(0, Math.round(now() - started)),
      };
    }),
  );
  return jsonResponse(200, { ip, family, results } satisfies ProbeResponse);
}

/** The requested ports, or `null` for anything but a small list of valid ones. */
export function parsePorts(body: string | null): number[] | null {
  if (body === null) return null;
  let value: unknown;
  try {
    value = JSON.parse(body);
  } catch {
    return null;
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const keys = Object.keys(value);
  if (keys.length !== 1 || keys[0] !== "ports") return null;
  const ports = (value as { ports: unknown }).ports;
  if (!Array.isArray(ports) || ports.length === 0 || ports.length > MAX_PORTS) return null;
  if (!ports.every((port) => Number.isInteger(port) && port >= MIN_PORT && port <= 65535)) {
    return null;
  }
  return [...new Set(ports as number[])];
}

async function readBody(request: Request): Promise<string | null> {
  const declared = Number(request.headers.get("content-length") ?? "0");
  if (declared > MAX_BODY_BYTES) return null;
  const text = await request.text();
  return text.length > MAX_BODY_BYTES ? null : text;
}

function errorResponse(status: number, code: ErrorCode): Response {
  return jsonResponse(status, { error: { code } });
}

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: {
      "cache-control": "no-store",
      "content-type": "application/json; charset=utf-8",
    },
  });
}
