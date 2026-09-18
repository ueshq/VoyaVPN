/**
 * VoyaVPN reachability probe: tells a self-hosted node whether a device on the
 * internet can open a TCP connection to it. See `probe.ts` for the contract
 * and `docs/release/self-host-probe-worker.md` for deployment.
 */

import { connect } from "cloudflare:sockets";

import { handleRequest, type ProbeOutcome } from "./probe";

type Env = {
  PROBE_LIMITER?: RateLimit;
};

/** @public The Workers runtime calls this module's default export. */
export default {
  fetch(request, env) {
    return handleRequest(request, { dial: dialTcp, limiter: env.PROBE_LIMITER });
  },
} satisfies ExportedHandler<Env>;

async function dialTcp(address: string, port: number, timeoutMs: number): Promise<ProbeOutcome> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const socket = connect({ hostname: address, port });
  const timeout = new Promise<ProbeOutcome>((resolve) => {
    timer = setTimeout(() => resolve("timeout"), timeoutMs);
  });
  try {
    return await Promise.race([socket.opened.then((): ProbeOutcome => "connected"), timeout]);
  } catch (error) {
    return /refused/i.test(String(error)) ? "refused" : "error";
  } finally {
    clearTimeout(timer);
    socket.close().catch(() => undefined);
  }
}
