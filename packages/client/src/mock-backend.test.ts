import { beforeEach, describe, expect, it, vi } from "vitest";

import type { InvalidateEvent, TransientStreamEvent } from "@voya/contracts";

import { appErrorOfKind } from "./errors";
import { createMockBackend } from "./mock-backend";
import { makePolicyGroupEntry, makeProfileEntry, makeSubscription } from "./mock-seed";

describe("createMockBackend", () => {
  let backend: ReturnType<typeof createMockBackend>;

  beforeEach(() => {
    backend = createMockBackend();
  });

  it("implements the whole command surface, so a missing one is a typed error", async () => {
    // The stub comes from the generated wire table, so a command added in Rust
    // reaches a screen as `unsupported` rather than as a TypeError.
    await expect(backend.commands.scanScreenQr()).rejects.toSatisfy(
      (error: unknown) => appErrorOfKind(error, "unsupported") !== null,
    );
  });

  it("serves the node list, subscriptions and settings it was seeded with", async () => {
    const seeded = createMockBackend({
      profiles: [makeProfileEntry(0, { remarks: "Tokyo" })],
      subscriptions: [makeSubscription(0, { remarks: "Work" })],
    });

    await expect(seeded.commands.listProfileSummaries()).resolves.toMatchObject({
      entries: [{ profile: { remarks: "Tokyo" } }],
      undecodableProfiles: 0,
    });
    await expect(seeded.commands.listSubscriptions()).resolves.toMatchObject([
      { remarks: "Work" },
    ]);
    await expect(seeded.commands.loadUiPreferences()).resolves.toEqual({
      language: "en",
      theme: "system",
    });
  });

  it("announces a committed mutation the way the backend does", async () => {
    const invalidations = vi.fn<(event: InvalidateEvent) => void>();
    backend.on("invalidateEvent", invalidations);

    await backend.commands.setActiveProfile("profile-1");

    expect(invalidations).toHaveBeenCalledOnce();
    expect(invalidations.mock.calls[0]?.[0].keys.map((key) => key.scope.kind)).toEqual([
      "profiles",
      "policyGroups",
      "appSettings",
    ]);
    const { entries } = await backend.commands.listProfileSummaries();
    expect(entries.filter((entry) => entry.isActive).map((entry) => entry.profile.id)).toEqual([
      "profile-1",
    ]);
  });

  it("rejects a switch to a node it does not have, with the entity and id", async () => {
    await expect(backend.commands.setActiveProfile("ghost")).rejects.toSatisfy(
      (error: unknown) =>
        appErrorOfKind(error, "notFound")?.kind.entity === "profile"
        && appErrorOfKind(error, "notFound")?.kind.id === "ghost",
    );
  });

  it("publishes the core state a connect and disconnect produce", async () => {
    const transient = vi.fn<(event: TransientStreamEvent) => void>();
    const unsubscribe = backend.on("transientStreamEvent", transient);

    await backend.commands.setActiveProfile("profile-0");
    await expect(backend.commands.connectActiveProfile()).resolves.toMatchObject({
      activeProfileId: "profile-0",
      state: "connected",
    });
    await expect(backend.commands.runtimeStatus()).resolves.toMatchObject({ state: "connected" });
    await expect(backend.commands.disconnectCore()).resolves.toMatchObject({
      state: "disconnected",
    });

    expect(transient.mock.calls.map(([event]) => event.kind)).toEqual(["coreState", "coreState"]);

    unsubscribe();
    await backend.commands.connectActiveProfile();
    expect(transient).toHaveBeenCalledTimes(2);
  });

  it("imports share links as nodes and subscription URLs as sources", async () => {
    const result = await backend.commands.importProfilesFromText(
      "vless://token@example.test:443#%E6%9D%B1%E4%BA%AC\nhttps://example.test/sub",
      null,
    );

    expect(result).toMatchObject({ imported: 1, parsed: 2 });
    expect(result.addedSubscriptionIds).toHaveLength(1);
    const { entries } = await backend.commands.listProfileSummaries();
    expect(entries.at(-1)?.profile.remarks).toBe("東京");
  });

  it("brings a subscription's nodes in only once it is updated", async () => {
    const seeded = createMockBackend({
      profiles: [],
      subscriptions: [makeSubscription(0)],
    });

    await expect(
      seeded.commands.updateSubscriptions("subscription-0", false, null),
    ).resolves.toMatchObject({ imported: 1, skipped: 0 });
    await expect(seeded.commands.listProfileSummaries()).resolves.toMatchObject({
      entries: [{ profile: { subscriptionId: "subscription-0" } }],
    });
  });

  it("reports the running group's live member only while a group is active", async () => {
    await expect(backend.commands.policyGroupRuntime()).resolves.toBeNull();

    const withGroup = createMockBackend({
      policyGroups: [makePolicyGroupEntry(0, { selectedProfileId: "profile-1" }, true)],
    });

    await expect(withGroup.commands.policyGroupRuntime()).resolves.toMatchObject({
      groupId: "group-0",
      nowProfileId: "profile-1",
    });
  });

  it("records every call, in order, with its arguments", async () => {
    await backend.commands.setLogStreaming(true);
    await backend.commands.runtimeStatus();

    expect(backend.state.calls).toEqual([
      { args: [true], command: "setLogStreaming" },
      { args: [], command: "runtimeStatus" },
    ]);
    expect(backend.state.logStreaming).toBe(true);
  });
});

describe("the routing surface", () => {
  it("starts from the seeded default profile and saves a rule into it", async () => {
    const backend = createMockBackend();

    const [routing] = await backend.commands.listRoutings();
    expect(routing).toMatchObject({ isActive: true, remarks: "Default", rules: [] });

    const saved = await backend.commands.saveRoutingRule(routing.id, {
      domain: ["example.test"],
      enabled: true,
      id: "",
      inboundTags: null,
      ip: null,
      kind: null,
      network: null,
      outbound: "proxy",
      port: null,
      process: null,
      protocol: null,
      remarks: "Office",
      scope: null,
    });

    expect(saved.rules).toMatchObject([{ id: "rule-0", remarks: "Office" }]);
    await expect(backend.commands.listRoutings()).resolves.toMatchObject([
      { rules: [{ remarks: "Office" }] },
    ]);

    await expect(backend.commands.deleteRoutingRules(routing.id, ["rule-0"])).resolves.toMatchObject(
      { rules: [] },
    );
  });

  it("refuses a rule for a routing profile it does not have", async () => {
    const backend = createMockBackend();

    await expect(
      backend.commands.deleteRoutingRules("ghost", ["rule-0"]),
    ).rejects.toSatisfy(
      (error: unknown) => appErrorOfKind(error, "notFound")?.kind.entity === "routing",
    );
  });
});

describe("the settings surface", () => {
  it("round-trips a saved bundle and announces every cache it feeds", async () => {
    const backend = createMockBackend();
    const invalidations = vi.fn<(event: InvalidateEvent) => void>();
    backend.on("invalidateEvent", invalidations);

    const settings = await backend.commands.loadAppSettings();
    await backend.commands.saveAppSettings({
      ...settings,
      behavior: { ...settings.behavior, autoCheckIp: false },
    });

    await expect(backend.commands.loadAppSettings()).resolves.toMatchObject({
      behavior: { autoCheckIp: false },
    });
    // The bundle is a projection of the whole config, so every surface derived
    // from it goes stale at once.
    expect(invalidations.mock.calls[0]?.[0].keys.map((key) => key.scope.kind)).toEqual([
      "appSettings",
      "uiPreferences",
      "dns",
      "connectionMode",
    ]);
  });

  it("keeps DNS and the bundle agreeing after a DNS-only save", async () => {
    const backend = createMockBackend();
    const dns = await backend.commands.loadDnsSettings();

    await backend.commands.saveDnsSettings({ ...dns, direct: "9.9.9.9" });

    await expect(backend.commands.loadDnsSettings()).resolves.toMatchObject({ direct: "9.9.9.9" });
    await expect(backend.commands.loadAppSettings()).resolves.toMatchObject({
      dns: { direct: "9.9.9.9" },
    });
  });

  it("offers the tunnel as the only capture path", async () => {
    const backend = createMockBackend();

    await expect(backend.commands.connectionModeStatus()).resolves.toEqual({
      mode: "vpn",
      processRulesEffective: false,
      processRulesSupported: false,
      systemProxyAvailable: false,
      vpnAvailable: true,
    });
  });
});
