import { describe, expect, it, vi } from "vitest";

import { addressFamily, isPublicAddress } from "../src/address";
import { handleRequest, parsePorts, type Dialer, type ProbeOutcome } from "../src/probe";

function probeRequest(body: unknown, headers: Record<string, string> = {}, init: RequestInit = {}) {
  return new Request("https://probe.example/v1/probe", {
    method: "POST",
    body: typeof body === "string" ? body : JSON.stringify(body),
    headers: { "CF-Connecting-IP": "203.0.113.7", ...headers },
    ...init,
  });
}

function dialer(outcomes: Record<number, ProbeOutcome>): Dialer {
  return vi.fn(async (_address: string, port: number) => outcomes[port] ?? "timeout");
}

async function body(response: Response) {
  return (await response.json()) as Record<string, unknown>;
}

describe("probe handler", () => {
  it("connects back to the caller on every requested port", async () => {
    const dial = dialer({ 42443: "connected", 42444: "refused" });
    let clock = 1000;
    const response = await handleRequest(probeRequest({ ports: [42443, 42444] }), {
      dial,
      now: () => (clock += 50),
    });

    expect(response.status).toBe(200);
    expect(response.headers.get("cache-control")).toBe("no-store");
    expect(dial).toHaveBeenCalledWith("203.0.113.7", 42443, 4000);
    expect(await body(response)).toEqual({
      ip: "203.0.113.7",
      family: "ipv4",
      results: [
        { port: 42443, reachable: true, reason: "connected", elapsedMs: expect.any(Number) },
        { port: 42444, reachable: false, reason: "refused", elapsedMs: expect.any(Number) },
      ],
    });
  });

  it("probes IPv6 callers over IPv6", async () => {
    const dial = dialer({ 42443: "timeout" });
    const response = await handleRequest(
      probeRequest({ ports: [42443] }, { "CF-Connecting-IP": "2606:4700::1111" }),
      { dial },
    );
    expect(await body(response)).toMatchObject({ family: "ipv6", ip: "2606:4700::1111" });
    expect(dial).toHaveBeenCalledWith("2606:4700::1111", 42443, 4000);
  });

  it("never dials a private, loopback or missing caller address", async () => {
    for (const address of ["", "10.0.0.2", "127.0.0.1", "100.64.3.4", "::1", "fd00::1", "::ffff:192.168.1.1", "nonsense"]) {
      const dial = dialer({});
      const response = await handleRequest(probeRequest({ ports: [42443] }, { "CF-Connecting-IP": address }), { dial });
      expect(response.status, address).toBe(403);
      expect(await body(response)).toEqual({ error: { code: "probeDenied" } });
      expect(dial).not.toHaveBeenCalled();
    }
  });

  it("rejects anything but a short list of unprivileged ports", async () => {
    for (const payload of [
      "not json",
      [],
      { ports: [] },
      { ports: [80] },
      { ports: [70000] },
      { ports: [1.5] },
      { ports: [2000, 2001, 2002, 2003, 2004] },
      { ports: [2000], target: "198.51.100.1" },
      "x".repeat(300),
    ]) {
      const response = await handleRequest(probeRequest(payload), { dial: dialer({}) });
      expect(response.status, JSON.stringify(payload)).toBe(400);
      expect(await body(response)).toEqual({ error: { code: "invalid" } });
    }
    expect(parsePorts('{"ports":[2000,2000]}')).toEqual([2000]);
    expect(parsePorts(null)).toBeNull();
  });

  it("rate limits per caller before dialing", async () => {
    const dial = dialer({});
    const limit = vi.fn(async () => ({ success: false }));
    const response = await handleRequest(probeRequest({ ports: [42443] }), { dial, limiter: { limit } });
    expect(response.status).toBe(429);
    expect(limit).toHaveBeenCalledWith({ key: "203.0.113.7" });
    expect(dial).not.toHaveBeenCalled();
  });

  it("answers other paths and methods without dialing", async () => {
    const dial = dialer({});
    const wrongPath = await handleRequest(new Request("https://probe.example/"), { dial });
    expect(wrongPath.status).toBe(404);
    const wrongMethod = await handleRequest(new Request("https://probe.example/v1/probe"), { dial });
    expect(wrongMethod.status).toBe(405);
    expect(dial).not.toHaveBeenCalled();
  });
});

describe("caller address screening", () => {
  it("accepts only globally routable unicast", () => {
    for (const address of ["203.0.113.7", "8.8.8.8", "2606:4700::1111", "2001:4860::8888", "::ffff:8.8.8.8"]) {
      expect(isPublicAddress(address), address).toBe(true);
    }
    for (const address of [
      "0.1.2.3", "10.1.2.3", "100.100.100.200", "169.254.169.254", "172.20.0.1", "192.168.0.1", "224.0.0.1",
      "::", "fe80::1", "fc00::1", "ff02::1", "2001:db8::1", "2001::1", "2002::1", "1:2:3:4:5:6:7:8:9", "1::2::3",
    ]) {
      expect(isPublicAddress(address), address).toBe(false);
    }
    expect(addressFamily("2001:db8::1")).toBe("ipv6");
    expect(addressFamily("1.2.3")).toBeNull();
  });
});
