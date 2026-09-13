import type { Page } from "@playwright/test";

// `tsconfig.e2e.json` pulls `src/ipc/bindings.ts` into this project purely so
// the mock can be checked against the generated contract: every `satisfies`
// below is erased at runtime, but a backend DTO that gains, loses or retypes a
// field now fails `pnpm --filter @voya/desktop typecheck` instead of failing a
// smoke assertion with a confusing message.
import type {
  AppError,
  AppSettingsV1,
  AppUpdaterStatus,
  ConnectionModeStatus,
  DnsSettings,
  ExportProfilesResult,
  ImportProfilesResult,
  ProcessCandidate,
  ProxyConnectionsSnapshot,
  ProxyMonitorStatus,
  QrCodeImage,
  QrScanResult,
  ResourceUpdateFile,
  Routing_Serialize,
  RoutingRule,
  RuntimeStatusResponse,
  SpeedtestRunResult,
  SpeedtestStatus,
  Subscription,
  SubscriptionMetadata,
  SubscriptionUpdateResult,
  SystemProxyType,
  SystemProxyStatusResponse,
  TrafficMode,
  TrafficModeResponse,
  TunStatus,
  WindowChromeConfig,
} from "../../src/ipc/bindings";

export async function installTauriSmokeMock(
  page: Page,
  titleBarLayout: WindowChromeConfig["titleBarLayout"] = "none",
) {
  await page.addInitScript((titleBarLayout) => {
    type CommandArgs = Record<string, unknown>;
    type Profile = Record<string, unknown>;
    type ProfileRow = {
      profile: Profile;
      metrics: Record<string, unknown>;
      traffic: Record<string, unknown>;
      isActive: boolean;
    };
    // Hand-written copies of these two drifted from the backend; the generated
    // shapes are the contract, so alias them instead of restating them.
    type Routing = Routing_Serialize;
    type Rule = RoutingRule;
    // Everything the mock keeps between commands. Typing it against the
    // generated DTOs is what makes the per-command `satisfies` below cheap:
    // every handler that returns a slice of state is checked for free.
    type MockState = {
      calls: Array<{ command: string; args: CommandArgs }>;
      dns: DnsSettings;
      profiles: ProfileRow[];
      subscriptions: Subscription[];
      subscriptionMetadata: SubscriptionMetadata[];
      connections: ProxyConnectionsSnapshot;
      unhandled: string[];
      routings: Routing[];
      runtime: RuntimeStatusResponse;
      settings: AppSettingsV1;
      appliedSettings?: AppSettingsV1;
      sysProxy: SystemProxyStatusResponse;
      tun: TunStatus;
      failNextCommand: string | null;
      trafficModeFailure: "apply" | "close" | null;
      appliedTrafficMode: TrafficMode;
    };
    type Callback = (event: {
      id: number;
      event: string;
      payload: unknown;
    }) => void;
    type Listener = { eventId: number; eventName: string; handlerId: number };

    const callbacks = new Map<number, Callback>();
    const listeners: Listener[] = [];
    let nextCallbackId = 1;
    let nextProfileId = 1;
    let nextRoutingId = 1;
    let nextRuleId = 1;
    let windowMaximized = false;

    const state: MockState = {
      failNextCommand: null,
      trafficModeFailure: null,
      appliedTrafficMode: "rule",
      calls: [] as Array<{ command: string; args: CommandArgs }>,
      dns: makeDnsSettings(),
      profiles: [] as ProfileRow[],
      subscriptions: [],
      subscriptionMetadata: [],
      connections: makeConnectionsSnapshot(),
      // Every command the mock does not implement lands here so a smoke run can
      // fail loudly instead of silently exercising an error path (the way the
      // unmocked boot-time `get_window_chrome_config` used to).
      unhandled: [] as string[],
      routings: [makeRouting("routing-default", "Default routing", true)],
      runtime: {
        activeProfileId: null,
        mainPid: null,
        prePid: null,
        connectedDurationMs: null,
        activeTunBackend: null,
        runningCoreType: null,
        state: "disconnected",
      },
      settings: makeAppSettings(),
      sysProxy: {
        management: "automatic",
        observation: "unknown",
        manualCleanupRequired: false,
        effectiveMode: "forcedClear",
        exceptions: "",
        proxy: null as string | null,
        requestedMode: "forcedChange",
      },
      tun: {
        allowEnableTun: true,
        backend: "process",
        elevationGranted: true,
        enabled: false,
        expectedProviderPath: null as string | null,
        lastProviderError: null as string | null,
        nativeComponentReady: true,
        needsServiceInstall: false,
        needsVpnPermission: false,
        preflight: {
          notes: [] as string[],
          platform: "linux",
          routeRestoreNote: "Smoke mock does not mutate routes.",
          state: "ready",
          windowsCleanupDevices: [] as string[],
        },
        providerPathMismatch: false,
        providerState: "notApplicable",
        requiresElevation: false,
        resolvedProviderPath: null as string | null,
        restoreOnDisconnect: true,
      },
    };

    state.settings.dns = clone(state.dns);

    function connectionModeStatus(): ConnectionModeStatus {
      const mode = state.tun.enabled ? "vpn" : "systemProxy";

      return {
        mode,
        processRulesEffective: mode === "vpn",
        vpnAvailable: true,
      };
    }

    function settleManualProxy() {
      if (state.sysProxy.management !== "manual") return;
      const serving = state.runtime.state === "connected" && !state.tun.enabled;
      state.sysProxy.effectiveMode = "unchanged";
      state.sysProxy.proxy = serving ? "127.0.0.1:10808" : null;
    }

    const profileScopes = ["profiles"];
    const subscriptionScopes = ["subscriptions", "subscriptionMetadata"];
    const routingScopes = ["routings"];
    const proxyRuntimeScopes = ["proxyConnections"];
    const connectionModeScopes = ["connectionMode", "appSettings"];

    // Mirrors `voya_app::invalidation`: the real shell emits an InvalidateEvent
    // for every committed mutation, so the mock has to as well — the frontend
    // no longer invalidates for itself.
    const invalidationScopes: Record<string, string[]> = {
      delete_profiles: profileScopes,
      delete_routing_rules: routingScopes,
      delete_routings: routingScopes,
      delete_subscriptions: [...subscriptionScopes, ...profileScopes],
      import_profiles_from_text: [...subscriptionScopes, ...profileScopes],
      move_profile: profileScopes,
      move_routing_rule: routingScopes,
      proxy_close_connection: proxyRuntimeScopes,
      proxy_set_traffic_mode: [...proxyRuntimeScopes, "appSettings"],
      run_speedtest: profileScopes,
      apply_pending_settings: ["appSettings"],
      save_app_settings: [
        "appSettings",
        "uiPreferences",
        "dns",
        "connectionMode",
      ],
      save_dns_settings: ["dns", "appSettings"],
      save_profile: profileScopes,
      save_routing: routingScopes,
      save_routing_rule: routingScopes,
      save_subscription: subscriptionScopes,
      set_active_profile: [...profileScopes, "appSettings"],
      set_active_routing: [...routingScopes, "appSettings"],
      set_connection_mode: connectionModeScopes,
      set_system_proxy_mode: connectionModeScopes,
      set_tun_enabled: connectionModeScopes,
      update_subscriptions: [...subscriptionScopes, ...profileScopes],
    };

    function invoke(command: string, args: CommandArgs = {}) {
      const result = dispatch(command, args);
      const scopes = invalidationScopes[command];
      if (!scopes) {
        return result;
      }

      // Emitted once the command has "committed", exactly like the shell.
      return Promise.resolve(result).then(
        (value) => {
          emitEvent("invalidate-event", {
            keys: scopes.map((kind) => ({ reason: command, scope: { kind } })),
          });
          return value;
        },
        (error: unknown) => {
          // A live mode failure happens after persistence, so its cache changes
          // are announced on the error path as well.
          if (command === "proxy_set_traffic_mode") {
            emitEvent("invalidate-event", {
              keys: scopes.map((kind) => ({
                reason: command,
                scope: { kind },
              })),
            });
          }
          throw error;
        },
      );
    }

    function dispatch(command: string, args: CommandArgs = {}) {
      state.calls.push({ command, args });
      if (state.failNextCommand === command) {
        state.failNextCommand = null;
        return Promise.reject(new Error("Simulated failure"));
      }

      const manualIds = command === "delete_profiles"
        ? readStringArray(args, "indexIds")
        : command === "save_profile"
          ? [String(readRecord(args, "profile").id ?? "")]
          : command === "move_profile"
            ? [String(args.indexId ?? "")]
            : [];
      if (
        state.profiles.some(
          (row) =>
            manualIds.includes(String(row.profile.id)) &&
            row.profile.subscriptionId,
        ) ||
        (command === "save_profile" &&
          readRecord(args, "profile").subscriptionId) ||
        (command === "import_profiles_from_text" && args.subscriptionId)
      )
        return Promise.reject(new Error("Subscription nodes are read-only"));
      switch (command) {
        case "plugin:event|listen": {
          // The event name and handler id have to be recorded: `emit` must reach
          // only the listeners registered for that event, otherwise a
          // TransientStreamEvent payload would also be routed as an
          // InvalidateEvent and an AppEvent.
          const eventId = nextCallbackId++;
          listeners.push({
            eventId,
            eventName: String(args.event ?? ""),
            handlerId: Number(args.handler ?? 0),
          });
          return Promise.resolve(eventId);
        }
        case "plugin:event|unlisten": {
          const index = listeners.findIndex(
            (listener) => listener.eventId === Number(args.eventId ?? -1),
          );
          if (index >= 0) {
            listeners.splice(index, 1);
          }
          return Promise.resolve(null);
        }
        case "plugin:event|emit":
        case "plugin:event|emit_to":
        case "plugin:resources|close":
        case "plugin:window|close":
        case "plugin:window|minimize":
        case "plugin:window|start_dragging":
        case "set_window_acrylic":
          return Promise.resolve(null);
        case "plugin:window|is_maximized":
          return Promise.resolve(windowMaximized);
        case "plugin:window|toggle_maximize":
          windowMaximized = !windowMaximized;
          emitEvent("tauri://resize", { width: 1180, height: 760 });
          return Promise.resolve(null);
        case "plugin:app|version":
          return Promise.resolve("0.1.0");
        case "plugin:updater|check":
          return Promise.resolve({
            body: null,
            currentVersion: "0.1.0",
            date: null,
            rawJson: {
              downloadUrl: "https://cdn.voyavpn.test/stable/latest.json",
            },
            rid: 9001,
            version: "0.2.0",
          });
        case "plugin:updater|download_and_install":
        case "plugin:process|restart":
          return Promise.resolve(null);
        case "get_window_chrome_config":
          // Called on every boot by use-window-chrome; leaving it unmocked meant
          // the shell silently fell back through its catch on each smoke run.
          return Promise.resolve({
            titleBarLayout,
          } satisfies WindowChromeConfig);
        case "load_ui_preferences":
          return Promise.resolve(clone(state.settings.appearance));
        case "load_app_settings":
          return Promise.resolve(clone(state.settings));
        case "get_settings_apply_status": {
          const connected = state.runtime.state === "connected";
          const applied = state.appliedSettings ?? state.settings;
          const currentCore = {
            ...state.settings,
            appearance: applied.appearance,
            network: {
              ...state.settings.network,
              systemProxy: applied.network.systemProxy,
            },
          };
          const action = !connected
            ? "none"
            : JSON.stringify(currentCore) !== JSON.stringify(applied)
              ? "reconnect"
              : JSON.stringify(state.settings.network.systemProxy) !==
                  JSON.stringify(applied.network.systemProxy)
                ? "reapplyProxy"
                : "none";
          return Promise.resolve({ action, connected });
        }
        case "apply_pending_settings":
          state.appliedSettings = clone(state.settings);
          return Promise.resolve({
            action: "none",
            connected: state.runtime.state === "connected",
          });
        case "save_app_settings":
          state.appliedSettings ??= clone(state.settings);
          state.settings = cloneRecord(args.settings) as typeof state.settings;
          state.dns = clone(state.settings.dns);
          return Promise.resolve(clone(state.settings));
        case "runtime_status":
          return Promise.resolve(clone(state.runtime));
        case "connect_active_profile": {
          state.appliedSettings = clone(state.settings);
          const active =
            state.profiles.find((row) => row.isActive) ??
            state.profiles[0] ??
            null;
          state.runtime = {
            activeProfileId: active ? String(active.profile.id) : null,
            mainPid: 4242,
            prePid: null,
            connectedDurationMs: 0,
            activeTunBackend: null,
            runningCoreType: "singBox",
            state: "connected",
          };
          settleManualProxy();
          return Promise.resolve(clone(state.runtime));
        }
        case "disconnect_core":
          state.runtime = {
            activeProfileId: null,
            mainPid: null,
            prePid: null,
            connectedDurationMs: null,
            activeTunBackend: null,
            runningCoreType: null,
            state: "disconnected",
          };
          settleManualProxy();
          return Promise.resolve(clone(state.runtime));
        case "restart_core":
          state.runtime = {
            ...state.runtime,
            connectedDurationMs: 0,
            mainPid: 4243,
            state: "connected",
          };
          return Promise.resolve(clone(state.runtime));
        case "system_proxy_status":
          return Promise.resolve(clone(state.sysProxy));
        case "open_network_settings":
          return Promise.resolve(null);
        case "recheck_system_proxy":
          if (["clear", "otherProxy"].includes(state.sysProxy.observation)) {
            state.sysProxy.manualCleanupRequired = false;
          }
          return Promise.resolve(clone(state.sysProxy));
        case "set_system_proxy_mode":
          state.sysProxy = {
            ...state.sysProxy,
            effectiveMode: String(
              args.mode ?? "forcedClear",
            ) as SystemProxyType,
            requestedMode: String(
              args.mode ?? "forcedClear",
            ) as SystemProxyType,
          };
          settleManualProxy();
          return Promise.resolve(clone(state.sysProxy));
        case "tun_status":
          return Promise.resolve(clone(state.tun));
        case "tun_request_elevation":
          state.tun = {
            ...state.tun,
            elevationGranted: true,
            requiresElevation: false,
          };
          return Promise.resolve(clone(state.tun));
        case "set_tun_enabled":
          state.tun = { ...state.tun, enabled: Boolean(args.enabled) };
          return Promise.resolve(clone(state.tun));
        case "connection_mode_status":
          return Promise.resolve(connectionModeStatus());
        case "set_connection_mode": {
          const mode = String(args.mode ?? "systemProxy");
          if (mode === "vpn") {
            state.tun = { ...state.tun, enabled: true };
          } else {
            state.tun = { ...state.tun, enabled: false };
            state.sysProxy = {
              ...state.sysProxy,
              effectiveMode: "forcedChange",
              requestedMode: "forcedChange",
            };
          }
          settleManualProxy();
          return Promise.resolve(connectionModeStatus());
        }
        case "list_profiles":
          // The real command answers with the rows plus the number of stored
          // profiles the build could not decode; the fixture never seeds an
          // unreadable row, so the count is always zero here.
          return Promise.resolve({
            entries: filterProfiles(state.profiles, args.filter),
            undecodableProfiles: 0,
          });
        case "save_profile": {
          const row = upsertProfile(readRecord(args, "profile"));
          return Promise.resolve(clone(row));
        }
        case "set_active_profile": {
          const row = setActiveProfile(String(args.indexId ?? ""));
          return Promise.resolve(clone(row));
        }
        case "delete_profiles": {
          const ids = readStringArray(args, "indexIds");
          state.profiles = state.profiles.filter(
            (row) => !ids.includes(String(row.profile.id)),
          );
          if (
            state.runtime.activeProfileId &&
            ids.includes(state.runtime.activeProfileId)
          )
            void dispatch("disconnect_core", {});
          return Promise.resolve(ids.length);
        }
        case "move_profile":
          return Promise.resolve(clone(state.profiles));
        case "list_subscriptions":
          return Promise.resolve(clone(state.subscriptions));
        case "list_subscription_metadata":
          return Promise.resolve(clone(state.subscriptionMetadata));
        case "list_process_candidates":
          return Promise.resolve([] satisfies ProcessCandidate[]);
        case "save_subscription": {
          const source = cloneRecord(args.item) as Subscription;
          source.id ||= `sub-${state.subscriptions.length + 1}`;
          state.subscriptions = [
            ...state.subscriptions.filter((item) => item.id !== source.id),
            source,
          ];
          return Promise.resolve(clone(source));
        }
        case "delete_subscriptions": {
          const ids = readStringArray(args, "ids");
          const removed = state.profiles
            .filter((row) => ids.includes(String(row.profile.subscriptionId)))
            .map((row) => String(row.profile.id));
          state.subscriptions = state.subscriptions.filter(
            (item) => !ids.includes(item.id),
          );
          state.profiles = state.profiles.filter(
            (row) => !removed.includes(String(row.profile.id)),
          );
          if (
            state.runtime.activeProfileId &&
            removed.includes(state.runtime.activeProfileId)
          ) {
            void dispatch("disconnect_core", {});
            emitEvent("transient-stream-event", {
              kind: "coreState",
              payload: clone(state.runtime),
            });
          }
          return Promise.resolve(ids.length);
        }
        case "export_profile_share_links": {
          const indexIds = readStringArray(args, "indexIds");
          const links = indexIds.map((indexId) => {
            const profile = state.profiles.find(
              (item) => item.profile.id === indexId,
            )?.profile;
            if (!profile) {
              throw new Error(`missing profile ${indexId}`);
            }

            const protocol = readRecord(profile, "protocol");
            const server = readRecord(protocol, "server");
            return `vless://${encodeURIComponent(String(protocol.uuid ?? protocol.password ?? ""))}@${String(server.address)}:${String(server.port)}#${encodeURIComponent(String(profile.remarks))}`;
          });

          return Promise.resolve({
            count: links.length,
            format: "shareLinks",
            text: links.join("\n"),
          } satisfies ExportProfilesResult);
        }
        case "import_profiles_from_text": {
          const row = upsertProfile(importedProfile(String(args.text ?? "")));
          // Mirrors every field of the generated `ImportProfilesResult`; a
          // partial shape let the dialog paper over the contract with `??`.
          return Promise.resolve({
            deduped: 0,
            discardedNodeOverrides: 0,
            failed: 0,
            filtered: 0,
            imported: 1,
            importedProfileIds: [String(row.profile.id)],
            lineIssues: [],
            parsed: 1,
            removedDuplicates: 0,
            removedExisting: 0,
            skipped: 0,
            subscriptionId: nullableString(args.subscriptionId),
            updated: 0,
            updatedProfileIds: [],
          } satisfies ImportProfilesResult);
        }
        case "update_subscriptions": {
          const source = state.subscriptions.find(
            (item) => item.id === args.subscriptionId,
          );
          if (!source)
            return Promise.resolve({
              imported: 0,
              messages: [],
              removedExisting: 0,
              skipped: 0,
              updated: 0,
            } satisfies SubscriptionUpdateResult);
          const previous = state.profiles.find(
            (row) => row.profile.subscriptionId === source.id,
          );
          if (!previous)
            upsertProfile({
              ...importedProfile(
                "trojan://secret@source.test:443#Subscription%20node",
              ),
              subscriptionId: source.id,
            });
          state.subscriptionMetadata = [
            ...state.subscriptionMetadata.filter(
              (item) => item.subscriptionId !== source.id,
            ),
            {
              subscriptionId: source.id,
              lastUpdateAt: Math.floor(Date.now() / 1000),
              uploadBytes: null,
              downloadBytes: null,
              totalBytes: null,
              expireAt: null,
              profileTitle: null,
            },
          ];
          return Promise.resolve({
            imported: 1,
            messages: [],
            removedExisting: 0,
            skipped: 0,
            updated: 1,
          } satisfies SubscriptionUpdateResult);
        }
        case "run_speedtest":
          return Promise.resolve({
            cancelled: false,
            completedCount: 0,
            results: [],
            selectedCount: 0,
          } satisfies SpeedtestRunResult);
        case "cancel_speedtest":
        case "speedtest_status":
          return Promise.resolve({ running: false } satisfies SpeedtestStatus);
        case "list_routings":
          return Promise.resolve(clone(state.routings));
        case "save_routing": {
          const routing = upsertRouting(readRecord(args, "item"));
          return Promise.resolve(clone(routing));
        }
        case "set_active_routing": {
          state.routings = state.routings.map((routing) => ({
            ...routing,
            isActive: routing.id === args.id,
          }));
          return Promise.resolve(
            clone(
              state.routings.find((routing) => routing.id === args.id) ??
                state.routings[0],
            ),
          );
        }
        case "delete_routings": {
          const ids = readStringArray(args, "ids");
          state.routings = state.routings.filter(
            (routing) => !ids.includes(routing.id),
          );
          return Promise.resolve(ids.length);
        }
        case "save_routing_rule": {
          const routing =
            state.routings.find((item) => item.id === args.routingId) ??
            state.routings[0];
          const rule = normalizeRule(readRecord(args, "rule"));
          const existingIndex = routing.rules.findIndex(
            (item) => item.id === rule.id,
          );
          routing.rules =
            existingIndex >= 0
              ? routing.rules.map((item) => (item.id === rule.id ? rule : item))
              : [...routing.rules, rule];
          return Promise.resolve(clone(routing));
        }
        case "delete_routing_rules": {
          const routing =
            state.routings.find((item) => item.id === args.routingId) ??
            state.routings[0];
          const ids = readStringArray(args, "ruleIds");
          routing.rules = routing.rules.filter(
            (rule) => !ids.includes(rule.id),
          );
          return Promise.resolve(clone(routing));
        }
        case "move_routing_rule": {
          const routing =
            state.routings.find((item) => item.id === args.routingId) ??
            state.routings[0];
          return Promise.resolve(clone(routing));
        }
        case "load_dns_settings":
          return Promise.resolve(clone(state.dns));
        case "save_dns_settings":
          state.dns = mergeDeep(state.dns, readRecord(args, "settings"));
          state.settings.dns = clone(state.dns);
          return Promise.resolve(clone(state.dns));
        case "proxy_list_connections":
          return Promise.resolve(clone(state.connections));
        case "proxy_close_connection":
          state.connections.connections =
            args.connectionId == null
              ? []
              : state.connections.connections.filter(
                  (connection) => connection.id !== args.connectionId,
                );
          return Promise.resolve(clone(state.connections));
        case "proxy_set_traffic_mode":
          state.settings.proxy.trafficMode = String(
            args.mode ?? "rule",
          ) as TrafficMode;
          if (
            state.runtime.state === "connected" &&
            state.settings.proxy.trafficMode !== "unchanged"
          ) {
            const failure = state.trafficModeFailure;
            state.trafficModeFailure = null;
            if (failure !== "apply")
              state.appliedTrafficMode = state.settings.proxy.trafficMode;
            if (failure) {
              return Promise.reject({
                kind: { type: "network" },
                subsystem: "proxyRuntime",
                message:
                  failure === "apply"
                    ? "traffic mode was saved but could not be applied to the running core: simulated API failure"
                    : "traffic mode was applied, but existing connections could not be closed: simulated API failure",
              } satisfies AppError);
            }
            state.connections.connections = [];
          }
          return Promise.resolve({
            mode: state.settings.proxy.trafficMode,
          } satisfies TrafficModeResponse);
        case "proxy_start_monitor":
          return Promise.resolve({
            message: null,
            running: true,
            stale: false,
            state: "running",
          } satisfies ProxyMonitorStatus);
        case "proxy_stop_monitor":
          return Promise.resolve({
            message: null,
            running: false,
            stale: true,
            state: "stopped",
          } satisfies ProxyMonitorStatus);
        case "scan_screen_qr":
          return Promise.resolve({
            texts: ["vless://00000000-0000-0000-0000-000000000001@screen.example.test:443#Screen%20node"],
            status: "found", source: "screen", message: null, failureReason: null,
          } satisfies QrScanResult);
        case "read_clipboard_text":
          return Promise.resolve(
            " vless://00000000-0000-0000-0000-000000000002@clipboard.example.test:443#Clipboard%20direct ",
          );
        case "generate_qr_code":
          return Promise.resolve({
            mimeType: "image/svg+xml",
            svg: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect width="64" height="64" fill="white"/><rect x="8" y="8" width="16" height="16" fill="black"/><rect x="40" y="8" width="16" height="16" fill="black"/><rect x="8" y="40" width="16" height="16" fill="black"/><rect x="32" y="32" width="8" height="8" fill="black"/></svg>',
          } satisfies QrCodeImage);
        case "app_update_status":
          return Promise.resolve({
            currentVersion: "0.1.0",
            message: null,
            state: "ready",
          } satisfies AppUpdaterStatus);
        case "update_geo_assets":
          return Promise.resolve([
            { bytes: 1024, name: "geoip.db", usedProxy: false },
          ] satisfies ResourceUpdateFile[]);
        case "update_srs_assets":
          return Promise.resolve([
            { bytes: 512, name: "rules.srs", usedProxy: false },
          ] satisfies ResourceUpdateFile[]);
        default:
          state.unhandled.push(command);
          throw {
            kind: "state",
            message: `Unhandled smoke command: ${command}`,
          };
      }
    }

    window.__TAURI_INTERNALS__ = {
      invoke,
      metadata: {
        currentWindow: {
          label: "main",
        },
      },
      transformCallback(callback: Callback) {
        const id = nextCallbackId++;
        callbacks.set(id, callback);
        return id;
      },
      unregisterCallback(id: number) {
        callbacks.delete(id);
      },
    };
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener() {
        return undefined;
      },
    };
    window.__VOYA_SMOKE__ = { emit: emitEvent, state };

    function emitEvent(event: string, payload: unknown) {
      listeners
        .filter((listener) => listener.eventName === event)
        .forEach((listener) => {
          callbacks.get(listener.handlerId)?.({
            event,
            id: listener.handlerId,
            payload,
          });
        });
    }

    function upsertProfile(input: Record<string, unknown>) {
      const profile = normalizeProfile(input);
      const existingIndex = state.profiles.findIndex(
        (row) => row.profile.id === profile.id,
      );
      const existing =
        existingIndex >= 0 ? state.profiles[existingIndex] : null;
      const row = {
        isActive: existing?.isActive ?? false,
        profile,
        metrics: {
          delayMs: existing?.metrics.delayMs ?? -1,
          ipInfo: existing?.metrics.ipInfo ?? null,
          countryCode: null,
          outcome: existing?.metrics.outcome ?? null,
          sort: existing?.metrics.sort ?? state.profiles.length,
        },
        traffic: existing?.traffic ?? {
          date: 20260601,
          todayDownload: 0,
          todayUpload: 0,
          totalDownload: 0,
          totalUpload: 0,
        },
      };

      if (existingIndex >= 0) {
        state.profiles[existingIndex] = row;
      } else {
        state.profiles.push(row);
      }

      if (row.isActive) {
        setActiveProfile(String(row.profile.id));
      }

      return row;
    }

    function setActiveProfile(indexId: string) {
      state.profiles = state.profiles.map((row) => ({
        ...row,
        isActive: row.profile.id === indexId,
      }));
      const row =
        state.profiles.find((item) => item.profile.id === indexId) ?? null;
      return row;
    }

    function normalizeProfile(input: Record<string, unknown>): Profile {
      const id = String(input.id || `profile-smoke-${nextProfileId++}`);
      const protocol = cloneRecord(input.protocol);
      return {
        displayLog: Boolean(input.displayLog ?? true),
        id,
        protocol:
          Object.keys(protocol).length > 0
            ? protocol
            : {
                kind: "vless",
                server: { address: "smoke.example.test", port: 443 },
                uuid: "00000000-0000-4000-8000-000000000001",
                flow: null,
                encryption: "none",
              },
        remarks: String(input.remarks ?? "Smoke profile"),
        subscriptionId: nullableString(input.subscriptionId),
        tls:
          input.tls && typeof input.tls === "object" ? clone(input.tls) : null,
        transport:
          input.transport && typeof input.transport === "object"
            ? clone(input.transport)
            : null,
      };
    }

    function importedProfile(text: string): Profile {
      const remark = decodeURIComponent(
        text.split("#")[1] ?? "Smoke Imported VLESS",
      ).replaceAll("+", " ");
      const addressMatch = text.match(/@([^:/?#]+)(?::(\d+))?/u);

      return normalizeProfile({
        remarks: remark,
        protocol: {
          encryption: "none",
          flow: null,
          kind: "vless",
          server: {
            address: addressMatch?.[1] ?? "imported.example.test",
            port: Number(addressMatch?.[2] ?? 443),
          },
          uuid:
            text.match(/^vless:\/\/([^@]+)/u)?.[1] ??
            "00000000-0000-4000-8000-000000000002",
        },
        tls: text.includes("security=tls")
          ? {
              alpn: [],
              certificatePem: null,
              echConfig: [],
              mode: "tls",
              realityPublicKey: null,
              realityShortId: null,
              serverName: null,
            }
          : null,
        transport: text.includes("type=ws")
          ? { host: "cdn.example.test", kind: "websocket", path: "/ws" }
          : { header: null, host: null, kind: "tcp", path: null },
      });
    }

    function filterProfiles(rows: ProfileRow[], filter: unknown) {
      const needle = String(filter ?? "")
        .trim()
        .toLowerCase();
      if (!needle) {
        return clone(rows);
      }

      return clone(
        rows.filter((row) =>
          [
            row.profile.remarks,
            profileAddress(row.profile),
            row.profile.subscriptionId,
          ]
            .join(" ")
            .toLowerCase()
            .includes(needle),
        ),
      );
    }

    function profileAddress(profile: Profile) {
      const protocol = readRecord(profile, "protocol");
      const server = readRecord(protocol, "server");
      return String(server.address ?? protocol.source ?? "");
    }

    function upsertRouting(input: Record<string, unknown>): Routing {
      const id = String(input.id ?? `routing-smoke-${nextRoutingId++}`);
      const existingIndex = state.routings.findIndex(
        (routing) => routing.id === id,
      );
      const existing =
        existingIndex >= 0 ? state.routings[existingIndex] : null;
      const routing = {
        icon: String(input.icon ?? existing?.icon ?? ""),
        singboxRulesetPath: String(
          input.singboxRulesetPath ?? existing?.singboxRulesetPath ?? "",
        ),
        singboxDomainStrategy: String(
          input.singboxDomainStrategy ?? existing?.singboxDomainStrategy ?? "",
        ),
        enabled: Boolean(input.enabled ?? existing?.enabled ?? true),
        id,
        isActive: Boolean(existing?.isActive ?? state.routings.length === 0),
        locked: Boolean(input.locked ?? existing?.locked ?? false),
        remarks: String(input.remarks ?? existing?.remarks ?? "Smoke routing"),
        rules: existing?.rules ?? [],
        sort: Number(input.sort ?? existing?.sort ?? state.routings.length),
      };

      if (existingIndex >= 0) {
        state.routings[existingIndex] = routing;
      } else {
        state.routings.push(routing);
      }

      return routing;
    }

    function normalizeRule(input: Record<string, unknown>): Rule {
      return {
        domain: readNullableStringArray(input, "domain"),
        enabled: Boolean(input.enabled ?? true),
        id: String(input.id ?? `rule-smoke-${nextRuleId++}`),
        inboundTags: readNullableStringArray(input, "inboundTags"),
        ip: readNullableStringArray(input, "ip"),
        network: nullableString(input.network),
        outbound: nullableString(input.outbound ?? "proxy"),
        port: nullableString(input.port),
        process: readNullableStringArray(input, "process"),
        protocol: readNullableStringArray(input, "protocol"),
        remarks: nullableString(input.remarks ?? "Smoke rule"),
        scope: (input.scope ?? "routing") as Rule["scope"],
        kind: nullableString(input.kind),
      };
    }

    function makeRouting(
      id: string,
      remarks: string,
      active: boolean,
    ): Routing {
      return {
        icon: "",
        singboxRulesetPath: "",
        singboxDomainStrategy: "",
        enabled: true,
        id,
        isActive: active,
        locked: false,
        remarks,
        rules: [],
        sort: 0,
      };
    }

    function makeAppSettings(): AppSettingsV1 {
      return {
        schemaVersion: 1,
        appearance: { language: "en", theme: "system" },
        behavior: { autostart: false },
        core: {
          bindInterface: null as string | null,
          cacheFileEnabled: true,
          defaultAllowInsecure: false,
          defaultFingerprint: "chrome",
          defaultUserAgent: "",
          fragmentFallbackDelayMs: 500,
          tlsFragment: "off",
          logEnabled: false,
          logLevel: "warn",
          muxEnabled: false,
          sendThrough: null as string | null,
        },
        network: {
          inbounds: [
            {
              lanConnectionsAllowed: false,
              localPort: 10808,
              password: "",
              secondaryPortEnabled: false,
              separateLanPort: false,
              sniffingEnabled: true,
              username: "",
            },
          ],
          systemProxy: {
            bypassLocal: true,
            exceptions: "",
            mode: "forcedChange",
          },
          tun: {
            autoRoute: true,
            enabled: false,
            icmpRouting: "rule",
            mtu: 1500,
            ipv6Enabled: false,
            stack: "system",
            strictRoute: false,
          },
        },
        routing: { domainStrategy: "AsIs" },
        dns: {
          addCommonHosts: null,
          blockBindingQuery: null,
          bootstrap: null,
          direct: null,
          directExpectedIps: null,
          directStrategy: null,
          fakeIp: null,
          globalFakeIp: null,
          hosts: null,
          proxyStrategy: null,
          remote: null,
        },
        speedTest: {
          delayIntervalSeconds: null as number | null,
          ipLookupUrl: "",
          latencyUrl: "https://www.google.com/generate_204",
          pageSize: null as number | null,
          timeoutSeconds: 10,
        },
        multiplexing: { maxConnections: 4, padding: false, protocol: "h2mux" },
        hysteria: {
          downloadMbps: 100,
          hopIntervalSeconds: 30,
          uploadMbps: 100,
        },
        proxy: { trafficMode: "rule" },
      };
    }

    function makeDnsSettings(): DnsSettings {
      return {
        addCommonHosts: true,
        blockBindingQuery: false,
        bootstrap: "1.1.1.1",
        direct: "223.5.5.5",
        directExpectedIps: "",
        fakeIp: false,
        globalFakeIp: false,
        hosts: "",
        proxyStrategy: "UseIP",
        remote: "https://1.1.1.1/dns-query",
        directStrategy: "AsIs",
      };
    }

    function clone<T>(value: T): T {
      return value === undefined ? value : structuredClone(value);
    }

    function cloneRecord(value: unknown) {
      return value && typeof value === "object"
        ? clone(value as Record<string, unknown>)
        : {};
    }

    function mergeDeep<T extends Record<string, unknown>>(
      target: T,
      patch: Record<string, unknown>,
    ): T {
      const next = clone(target) as Record<string, unknown>;
      Object.entries(patch).forEach(([key, value]) => {
        if (
          value &&
          typeof value === "object" &&
          !Array.isArray(value) &&
          next[key] &&
          typeof next[key] === "object" &&
          !Array.isArray(next[key])
        ) {
          next[key] = mergeDeep(
            next[key] as Record<string, unknown>,
            value as Record<string, unknown>,
          );
        } else {
          next[key] = value;
        }
      });
      return next as T;
    }

    function readRecord(args: CommandArgs, key: string) {
      const value = args[key];
      return value && typeof value === "object"
        ? (value as Record<string, unknown>)
        : {};
    }

    function readArray(args: CommandArgs, key: string) {
      const value = args[key];
      return Array.isArray(value) ? value : [];
    }

    function makeConnectionsSnapshot(): ProxyConnectionsSnapshot {
      return {
        connections: [
          {
            chains: ["PROXY", "Smoke Node"],
            connectionType: "HTTPS",
            destination: "93.184.216.34",
            download: 2048,
            host: "smoke.example.test:443",
            id: "smoke-connection",
            network: "tcp",
            process: "smoke-app",
            processPath: "/usr/bin/smoke-app",
            rule: "Match",
            rulePayload: "",
            source: "127.0.0.1:54321",
            start: "2026-01-01T00:00:00Z",
            upload: 1024,
          },
        ],
        downloadTotal: 2048,
        uploadTotal: 1024,
      };
    }

    function readStringArray(args: CommandArgs, key: string) {
      return readArray(args, key).map(String);
    }

    function readNullableStringArray(
      input: Record<string, unknown>,
      key: string,
    ) {
      const value = input[key];
      if (!Array.isArray(value)) {
        return null;
      }
      return value.map(String);
    }

    function nullableString(value: unknown) {
      if (value === null || value === undefined || value === "") {
        return null;
      }
      return String(value);
    }
  }, titleBarLayout);
}

declare global {
  interface Window {
    __TAURI_INTERNALS__: {
      invoke: (
        command: string,
        args?: Record<string, unknown>,
      ) => Promise<unknown>;
      metadata: {
        currentWindow: {
          label: string;
        };
      };
      transformCallback: (
        callback: (event: {
          id: number;
          event: string;
          payload: unknown;
        }) => void,
      ) => number;
      unregisterCallback: (id: number) => void;
    };
    __VOYA_SMOKE__: {
      emit: (event: string, payload: unknown) => void;
      state: unknown;
    };
  }
}
