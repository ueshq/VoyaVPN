import type { Page } from "@playwright/test";

export async function installTauriSmokeMock(page: Page) {
  await page.addInitScript(() => {
    type CommandArgs = Record<string, unknown>;
    type Profile = Record<string, unknown>;
    type ProfileRow = {
      profile: Profile;
      profileEx: Record<string, unknown>;
      serverStat: Record<string, unknown>;
      isActive: boolean;
    };
    type Routing = {
      Id: string;
      Remarks: string;
      Url: string;
      RuleSet: Rule[];
      RuleNum: number;
      Enabled: boolean;
      Locked: boolean;
      CustomIcon: string;
      CustomRulesetPath4Singbox: string;
      DomainStrategy: string;
      DomainStrategy4Singbox: string;
      Sort: number;
      IsActive: boolean;
    };
    type Rule = {
      Id: string;
      Type?: string | null;
      Port?: string | null;
      Network?: string | null;
      InboundTag?: string[] | null;
      OutboundTag?: string | null;
      Ip?: string[] | null;
      Domain?: string[] | null;
      Protocol?: string[] | null;
      Process?: string[] | null;
      Enabled: boolean;
      Remarks?: string | null;
      RuleType?: number | null;
    };
    type Callback = (event: { id: number; event: string; payload: unknown }) => void;

    const callbacks = new Map<number, Callback>();
    let nextCallbackId = 1;
    let nextProfileId = 1;
    let nextRoutingId = 1;
    let nextRuleId = 1;

    const state = {
      appConfig: makeAppConfig(),
      autostart: {
        artifactKind: "linuxDesktopFile",
        artifactName: "VoyaVPN.desktop",
        artifactPath: "/home/smoke/.config/autostart/VoyaVPN.desktop",
        enabled: false,
        platform: "linux",
      },
      calls: [] as Array<{ command: string; args: CommandArgs }>,
      dns: makeDnsSettings(),
      hotkeys: makeHotkeyStatus(),
      profiles: [] as ProfileRow[],
      routings: [makeRouting("routing-default", "Default routing", true)],
      runtime: {
        activeProfileId: null as string | null,
        mainPid: null as number | null,
        prePid: null as number | null,
        runningCoreType: null as number | null,
        state: "disconnected",
      },
      sources: {
        geoSourceUrl: null as string | null,
        srsSourceUrl: null as string | null,
      },
      sysProxy: {
        effectiveMode: 0,
        exceptions: "",
        pacAvailable: false,
        pacUrl: null as string | null,
        proxy: null as string | null,
        requestedMode: 0,
      },
      tun: {
        allowEnableTun: true,
        enabled: false,
        preflight: {
          notes: [] as string[],
          platform: "linux",
          routeRestoreNote: "Smoke mock does not mutate routes.",
          state: "ready",
          windowsCleanupDevices: [] as string[],
        },
        requiresSudoPassword: false,
        restoreOnDisconnect: true,
        sudoPasswordPresent: false,
      },
      updates: {
        preRelease: false,
        targets: [
          {
            acquisition: "appPackage",
            coreType: null,
            id: "app",
            kind: "app",
            license: "MIT",
            name: "VoyaVPN",
            redistributeInInstaller: true,
            remarks: "application package update",
            selected: true,
            updateSupported: true,
          },
        ],
      },
      webDav: {
        DirName: null as string | null,
        Password: null as string | null,
        Url: null as string | null,
        UserName: null as string | null,
      },
    };

    function invoke(command: string, args: CommandArgs = {}) {
      state.calls.push({ command, args });

      switch (command) {
        case "plugin:event|listen":
          return Promise.resolve(nextCallbackId++);
        case "plugin:event|unlisten":
        case "plugin:event|emit":
        case "plugin:event|emit_to":
        case "plugin:resources|close":
          return Promise.resolve(null);
        case "plugin:app|version":
          return Promise.resolve("0.1.0");
        case "plugin:updater|check":
          return Promise.resolve({
            body: null,
            currentVersion: "0.1.0",
            date: null,
            rawJson: { downloadUrl: "https://cdn.voyavpn.test/stable/latest.json" },
            rid: 9001,
            version: "0.2.0",
          });
        case "plugin:updater|download_and_install":
        case "plugin:process|restart":
          return Promise.resolve(null);
        case "app_health":
          return Promise.resolve("ok");
        case "load_app_config":
          return Promise.resolve(clone(state.appConfig));
        case "save_app_config": {
          state.appConfig = mergeDeep(state.appConfig, readRecord(args, "config"));
          return Promise.resolve(clone(state.appConfig));
        }
        case "runtime_status":
          return Promise.resolve(clone(state.runtime));
        case "connect_active_profile": {
          const active = state.profiles.find((row) => row.isActive) ?? state.profiles[0] ?? null;
          state.runtime = {
            activeProfileId: active ? String(active.profile.IndexId) : null,
            mainPid: 4242,
            prePid: null,
            runningCoreType: 2,
            state: "connected",
          };
          return Promise.resolve(clone(state.runtime));
        }
        case "disconnect_core":
          state.runtime = {
            activeProfileId: null,
            mainPid: null,
            prePid: null,
            runningCoreType: null,
            state: "disconnected",
          };
          return Promise.resolve(clone(state.runtime));
        case "restart_core":
          state.runtime = {
            ...state.runtime,
            mainPid: 4243,
            state: "connected",
          };
          return Promise.resolve(clone(state.runtime));
        case "system_proxy_status":
          return Promise.resolve(clone(state.sysProxy));
        case "set_system_proxy_mode":
          state.sysProxy = {
            ...state.sysProxy,
            effectiveMode: Number(args.mode ?? 0),
            requestedMode: Number(args.mode ?? 0),
          };
          return Promise.resolve(clone(state.sysProxy));
        case "tun_status":
          return Promise.resolve(clone(state.tun));
        case "set_tun_enabled":
          state.tun = { ...state.tun, enabled: Boolean(args.enabled) };
          return Promise.resolve(clone(state.tun));
        case "sudo_begin_collection":
          return Promise.resolve({ requestId: null, state: "ready" });
        case "sudo_submit_password":
          state.tun = { ...state.tun, sudoPasswordPresent: true };
          return Promise.resolve({ requestId: args.requestId ?? null, state: "ready" });
        case "sudo_clear_password":
          state.tun = { ...state.tun, sudoPasswordPresent: false };
          return Promise.resolve(null);
        case "sudo_has_password":
          return Promise.resolve(state.tun.sudoPasswordPresent);
        case "list_profiles":
          return Promise.resolve(filterProfiles(state.profiles, args.filter));
        case "get_profile": {
          const row = state.profiles.find((item) => item.profile.IndexId === args.indexId) ?? null;
          return Promise.resolve(clone(row));
        }
        case "save_profile": {
          const row = upsertProfile(readRecord(args, "profile"));
          return Promise.resolve(clone(row));
        }
        case "save_group_profile": {
          const row = upsertProfile(readRecord(args, "profile"));
          return Promise.resolve(clone(row));
        }
        case "set_active_profile": {
          const row = setActiveProfile(String(args.indexId ?? ""));
          return Promise.resolve(clone(row));
        }
        case "delete_profiles": {
          const ids = readStringArray(args, "indexIds");
          state.profiles = state.profiles.filter((row) => !ids.includes(String(row.profile.IndexId)));
          return Promise.resolve(ids.length);
        }
        case "copy_profiles": {
          const ids = readStringArray(args, "indexIds");
          const copies = state.profiles
            .filter((row) => ids.includes(String(row.profile.IndexId)))
            .map((row) =>
              upsertProfile({
                ...row.profile,
                IndexId: undefined,
                Remarks: `${String(row.profile.Remarks)} Copy`,
              }),
            );
          return Promise.resolve(clone(copies));
        }
        case "move_profile":
        case "sort_profiles":
          return Promise.resolve(clone(state.profiles));
        case "dedupe_profiles":
          return Promise.resolve({ kept: state.profiles.length, removedIndexIds: [], total: state.profiles.length });
        case "list_group_child_candidates":
          return Promise.resolve(
            state.profiles.map((row) => ({
              address: row.profile.Address,
              configType: row.profile.ConfigType,
              indexId: row.profile.IndexId,
              isGroup: Number(row.profile.ConfigType) >= 101,
              reason: null,
              remarks: row.profile.Remarks,
              selectable: true,
              subid: row.profile.Subid,
            })),
          );
        case "validate_group_profile":
          return Promise.resolve({ childIndexIds: [], errors: [], normalizedChildItems: "", valid: true, warnings: [] });
        case "preview_group_profile":
          return Promise.resolve({
            singboxRoutes: [],
            validation: { childIndexIds: [], errors: [], normalizedChildItems: "", valid: true, warnings: [] },
          });
        case "list_subscriptions":
          return Promise.resolve([]);
        case "get_subscription":
          return Promise.resolve(null);
        case "save_subscription":
          return Promise.resolve({ Id: "sub-smoke", Remarks: "Smoke", Url: "", MoreUrl: "", Enabled: true, UserAgent: "", Sort: 0, UpdateTime: null });
        case "delete_subscriptions":
          return Promise.resolve(0);
        case "import_profiles_from_text": {
          const row = upsertProfile(importedProfile(String(args.text ?? "")));
          return Promise.resolve({ imported: 1, importedIndexIds: [row.profile.IndexId], removedExisting: 0, skipped: 0, subid: args.subid ?? null });
        }
        case "import_profiles_from_file":
          return Promise.resolve({ imported: 0, importedIndexIds: [], removedExisting: 0, skipped: 0, subid: args.subid ?? null });
        case "update_subscriptions":
        case "run_due_subscription_updates":
          return Promise.resolve({ imported: 0, messages: [], removedExisting: 0, skipped: 0, updated: 0 });
        case "run_speedtest":
          return Promise.resolve({ action: args.action, message: "smoke skipped real speedtest", requested: readStringArray(args, "indexIds").length, started: false });
        case "cancel_speedtest":
        case "speedtest_status":
          return Promise.resolve({ running: false });
        case "list_routings":
          return Promise.resolve(clone(state.routings));
        case "get_routing": {
          const routing = state.routings.find((item) => item.Id === args.id) ?? null;
          return Promise.resolve(clone(routing));
        }
        case "save_routing": {
          const routing = upsertRouting(readRecord(args, "item"));
          return Promise.resolve(clone(routing));
        }
        case "set_active_routing": {
          state.routings = state.routings.map((routing) => ({ ...routing, IsActive: routing.Id === args.id }));
          return Promise.resolve(clone(state.routings.find((routing) => routing.Id === args.id) ?? state.routings[0]));
        }
        case "delete_routings": {
          const ids = readStringArray(args, "ids");
          state.routings = state.routings.filter((routing) => !ids.includes(routing.Id));
          return Promise.resolve(ids.length);
        }
        case "save_routing_rule": {
          const routing = state.routings.find((item) => item.Id === args.routingId) ?? state.routings[0];
          const rule = normalizeRule(readRecord(args, "rule"));
          const existingIndex = routing.RuleSet.findIndex((item) => item.Id === rule.Id);
          routing.RuleSet =
            existingIndex >= 0
              ? routing.RuleSet.map((item) => (item.Id === rule.Id ? rule : item))
              : [...routing.RuleSet, rule];
          routing.RuleNum = routing.RuleSet.length;
          return Promise.resolve(clone(routing));
        }
        case "delete_routing_rules": {
          const routing = state.routings.find((item) => item.Id === args.routingId) ?? state.routings[0];
          const ids = readStringArray(args, "ruleIds");
          routing.RuleSet = routing.RuleSet.filter((rule) => !ids.includes(rule.Id));
          routing.RuleNum = routing.RuleSet.length;
          return Promise.resolve(clone(routing));
        }
        case "move_routing_rule": {
          const routing = state.routings.find((item) => item.Id === args.routingId) ?? state.routings[0];
          return Promise.resolve(clone(routing));
        }
        case "import_routing_templates":
          return Promise.resolve(clone(state.routings));
        case "apply_regional_preset":
          return Promise.resolve({
            fallbackCustomDnsEnabled: false,
            geoSourceUrl: state.sources.geoSourceUrl,
            presetType: args.presetType ?? 0,
            routeRulesTemplateSourceUrl: state.appConfig.ConstItem.RouteRulesTemplateSourceUrl,
            simpleDnsFetched: false,
            singboxDnsFetched: false,
            srsSourceUrl: state.sources.srsSourceUrl,
          });
        case "load_dns_settings":
          return Promise.resolve(clone(state.dns));
        case "save_dns_settings":
          state.dns = mergeDeep(state.dns, readRecord(args, "settings"));
          return Promise.resolve(clone(state.dns));
        case "clash_list_proxies":
          return Promise.resolve({
            allNodes: [
              { active: true, delay: 23, delayLabel: "23 ms", name: "Smoke Node", proxyType: "VLESS", testable: true, udp: true },
            ],
            groups: [
              {
                name: "PROXY",
                nodes: [
                  { active: true, delay: 23, delayLabel: "23 ms", name: "Smoke Node", proxyType: "VLESS", testable: true, udp: true },
                ],
                now: "Smoke Node",
                proxyType: "Selector",
              },
            ],
            ruleMode: 0,
          });
        case "clash_test_delay":
          return Promise.resolve(readStringArray(args, "proxyNames").map((name) => ({ delay: 23, message: null, name })));
        case "clash_select_proxy":
          return invoke("clash_list_proxies", args);
        case "clash_list_connections":
        case "clash_close_connection":
          return Promise.resolve({ connections: [], downloadTotal: 0, uploadTotal: 0 });
        case "clash_set_rule_mode":
          return Promise.resolve(clone(state.appConfig));
        case "clash_reload_config":
          return Promise.resolve(null);
        case "clash_start_monitor":
        case "clash_stop_monitor":
          return Promise.resolve({ running: false });
        case "autostart_status":
          return Promise.resolve(clone(state.autostart));
        case "set_autostart_enabled":
          state.autostart = { ...state.autostart, enabled: Boolean(args.enabled) };
          return Promise.resolve(clone(state.autostart));
        case "global_hotkey_status":
          return Promise.resolve(clone(state.hotkeys));
        case "save_global_hotkeys":
          state.hotkeys = { ...state.hotkeys, settings: readArray(args, "settings") };
          return Promise.resolve(clone(state.hotkeys));
        case "load_ruleset_geo_sources":
          return Promise.resolve(clone(state.sources));
        case "save_ruleset_geo_sources":
          state.sources = {
            geoSourceUrl: readRecord(args, "settings").geoSourceUrl as string | null,
            srsSourceUrl: readRecord(args, "settings").srsSourceUrl as string | null,
          };
          return Promise.resolve(clone(state.sources));
        case "generate_qr_code":
          return Promise.resolve({
            mimeType: "image/svg+xml",
            svg: "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 64 64\"><rect width=\"64\" height=\"64\" fill=\"white\"/><rect x=\"8\" y=\"8\" width=\"16\" height=\"16\" fill=\"black\"/><rect x=\"40\" y=\"8\" width=\"16\" height=\"16\" fill=\"black\"/><rect x=\"8\" y=\"40\" width=\"16\" height=\"16\" fill=\"black\"/><rect x=\"32\" y=\"32\" width=\"8\" height=\"8\" fill=\"black\"/></svg>",
          });
        case "backup_status":
          return Promise.resolve({
            backupDir: "/tmp/voyavpn-smoke/backups",
            defaultBackupPath: "/tmp/voyavpn-smoke/backups/smoke.zip",
            webDavItem: clone(state.webDav),
          });
        case "backup_save_webdav_settings":
          state.webDav = mergeDeep(state.webDav, readRecord(args, "settings"));
          return Promise.resolve(clone(state.webDav));
        case "backup_create_local":
          return Promise.resolve({ bytes: 1024, message: "Smoke backup created", path: args.outputPath ?? "/tmp/voyavpn-smoke/backups/smoke.zip" });
        case "backup_restore_local":
          return Promise.resolve({ message: "Smoke backup restored", path: args.inputPath, restoredConfig: clone(state.appConfig) });
        case "backup_webdav_check":
          return Promise.resolve({ bytes: null, message: "Smoke WebDAV check ok", path: null });
        case "backup_webdav_push":
          return Promise.resolve({ bytes: 1024, message: "Smoke WebDAV upload ok", path: null, remotePath: "/VoyaVPN/smoke.zip" });
        case "backup_webdav_pull":
          return Promise.resolve({ message: "Smoke WebDAV restore ok", path: "/tmp/voyavpn-smoke/webdav.zip", restoredConfig: clone(state.appConfig) });
        case "app_update_status":
          return Promise.resolve({ currentVersion: "0.1.0", message: null, state: "ready" });
        case "manual_app_update_links":
          return Promise.resolve({
            arch: "x64",
            channel: "stable",
            currentVersion: "0.1.0",
            downloads: [],
            hasUpdate: false,
            releaseIndexUrl: "https://cdn.voyavpn.test/stable/release-index.json",
            remoteVersion: null,
            target: "linux",
          });
        case "record_app_update_diagnostic":
          return Promise.resolve(null);
        case "update_status":
          return Promise.resolve(clone(state.updates));
        case "save_update_preferences":
          state.updates = { ...state.updates, preRelease: Boolean(args.preRelease) };
          return Promise.resolve(clone(state.updates));
        case "check_updates":
        case "download_updates":
          return Promise.resolve({ preRelease: Boolean(args.preRelease), results: [], targets: clone(state.updates.targets) });
        case "ipc_demo_round_trip":
          return Promise.resolve({ echoedMessage: readRecord(args, "request").message ?? "", messageLength: String(readRecord(args, "request").message ?? "").length });
        default:
          throw { kind: "state", message: `Unhandled smoke command: ${command}` };
      }
    }

    window.__TAURI_INTERNALS__ = {
      invoke,
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
    window.__VOYA_SMOKE__ = {
      emit(event: string, payload: unknown) {
        callbacks.forEach((callback, id) => callback({ event, id, payload }));
      },
      state,
    };

    function upsertProfile(input: Record<string, unknown>) {
      const profile = normalizeProfile(input);
      const existingIndex = state.profiles.findIndex((row) => row.profile.IndexId === profile.IndexId);
      const existing = existingIndex >= 0 ? state.profiles[existingIndex] : null;
      const row = {
        isActive: existing?.isActive ?? state.profiles.length === 0,
        profile,
        profileEx: {
          Delay: existing?.profileEx.Delay ?? -1,
          IndexId: profile.IndexId,
          IpInfo: existing?.profileEx.IpInfo ?? null,
          Message: existing?.profileEx.Message ?? null,
          Sort: existing?.profileEx.Sort ?? state.profiles.length,
          Speed: existing?.profileEx.Speed ?? null,
        },
        serverStat: existing?.serverStat ?? {
          DateNow: 20260601,
          IndexId: profile.IndexId,
          TodayDown: 0,
          TodayUp: 0,
          TotalDown: 0,
          TotalUp: 0,
        },
      };

      if (existingIndex >= 0) {
        state.profiles[existingIndex] = row;
      } else {
        state.profiles.push(row);
      }

      if (row.isActive) {
        setActiveProfile(String(row.profile.IndexId));
      }

      return row;
    }

    function setActiveProfile(indexId: string) {
      state.profiles = state.profiles.map((row) => ({ ...row, isActive: row.profile.IndexId === indexId }));
      const row = state.profiles.find((item) => item.profile.IndexId === indexId) ?? null;
      state.appConfig.IndexId = row ? String(row.profile.IndexId) : "";
      return row;
    }

    function normalizeProfile(input: Record<string, unknown>): Profile {
      const configType = Number(input.ConfigType ?? 5);
      const id = String(input.IndexId ?? `profile-smoke-${nextProfileId++}`);

      return {
        Address: String(input.Address ?? "smoke.example.test"),
        AllowInsecure: String(input.AllowInsecure ?? "false"),
        Alpn: String(input.Alpn ?? ""),
        Cert: String(input.Cert ?? ""),
        CertSha: String(input.CertSha ?? ""),
        ConfigType: configType,
        ConfigVersion: Number(input.ConfigVersion ?? 4),
        CoreType: input.CoreType ?? null,
        DisplayLog: Boolean(input.DisplayLog ?? true),
        EchConfigList: String(input.EchConfigList ?? ""),
        Finalmask: String(input.Finalmask ?? ""),
        Fingerprint: String(input.Fingerprint ?? ""),
        IndexId: id,
        IsSub: Boolean(input.IsSub ?? false),
        Mldsa65Verify: String(input.Mldsa65Verify ?? ""),
        MuxEnabled: Boolean(input.MuxEnabled ?? false),
        Network: String(input.Network ?? "tcp"),
        Password: String(input.Password ?? "00000000-0000-4000-8000-000000000001"),
        Port: Number(input.Port ?? 443),
        PreSocksPort: input.PreSocksPort ?? null,
        ProtocolExtra: cloneRecord(input.ProtocolExtra),
        PublicKey: String(input.PublicKey ?? ""),
        Remarks: String(input.Remarks ?? "Smoke profile"),
        ShortId: String(input.ShortId ?? ""),
        Sni: String(input.Sni ?? ""),
        SpiderX: String(input.SpiderX ?? ""),
        StreamSecurity: String(input.StreamSecurity ?? ""),
        Subid: String(input.Subid ?? ""),
        TransportExtra: cloneRecord(input.TransportExtra),
        Username: String(input.Username ?? ""),
      };
    }

    function importedProfile(text: string): Profile {
      const remark = decodeURIComponent(text.split("#")[1] ?? "Smoke Imported VLESS").replaceAll("+", " ");
      const addressMatch = text.match(/@([^:/?#]+)(?::(\d+))?/u);

      return normalizeProfile({
        Address: addressMatch?.[1] ?? "imported.example.test",
        ConfigType: 5,
        Network: text.includes("type=ws") ? "ws" : "tcp",
        Password: text.match(/^vless:\/\/([^@]+)/u)?.[1] ?? "00000000-0000-4000-8000-000000000002",
        Port: Number(addressMatch?.[2] ?? 443),
        Remarks: remark,
        StreamSecurity: text.includes("security=tls") ? "tls" : "",
        TransportExtra: {
          Host: "cdn.example.test",
          Path: "/ws",
        },
      });
    }

    function filterProfiles(rows: ProfileRow[], filter: unknown) {
      const needle = String(filter ?? "").trim().toLowerCase();
      if (!needle) {
        return clone(rows);
      }

      return clone(
        rows.filter((row) =>
          [row.profile.Remarks, row.profile.Address, row.profile.Subid]
            .join(" ")
            .toLowerCase()
            .includes(needle),
        ),
      );
    }

    function upsertRouting(input: Record<string, unknown>) {
      const id = String(input.Id ?? `routing-smoke-${nextRoutingId++}`);
      const existingIndex = state.routings.findIndex((routing) => routing.Id === id);
      const existing = existingIndex >= 0 ? state.routings[existingIndex] : null;
      const routing = {
        CustomIcon: String(input.CustomIcon ?? existing?.CustomIcon ?? ""),
        CustomRulesetPath4Singbox: String(input.CustomRulesetPath4Singbox ?? existing?.CustomRulesetPath4Singbox ?? ""),
        DomainStrategy: String(input.DomainStrategy ?? existing?.DomainStrategy ?? "AsIs"),
        DomainStrategy4Singbox: String(input.DomainStrategy4Singbox ?? existing?.DomainStrategy4Singbox ?? ""),
        Enabled: Boolean(input.Enabled ?? existing?.Enabled ?? true),
        Id: id,
        IsActive: Boolean(input.IsActive ?? existing?.IsActive ?? state.routings.length === 0),
        Locked: Boolean(input.Locked ?? existing?.Locked ?? false),
        Remarks: String(input.Remarks ?? existing?.Remarks ?? "Smoke routing"),
        RuleNum: existing?.RuleSet.length ?? 0,
        RuleSet: existing?.RuleSet ?? [],
        Sort: Number(input.Sort ?? existing?.Sort ?? state.routings.length),
        Url: String(input.Url ?? existing?.Url ?? ""),
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
        Domain: readNullableStringArray(input, "Domain"),
        Enabled: Boolean(input.Enabled ?? true),
        Id: String(input.Id ?? `rule-smoke-${nextRuleId++}`),
        InboundTag: readNullableStringArray(input, "InboundTag"),
        Ip: readNullableStringArray(input, "Ip"),
        Network: nullableString(input.Network),
        OutboundTag: nullableString(input.OutboundTag ?? "proxy"),
        Port: nullableString(input.Port),
        Process: readNullableStringArray(input, "Process"),
        Protocol: readNullableStringArray(input, "Protocol"),
        Remarks: nullableString(input.Remarks ?? "Smoke rule"),
        RuleType: Number(input.RuleType ?? 1),
        Type: nullableString(input.Type),
      };
    }

    function makeRouting(id: string, remarks: string, active: boolean): Routing {
      return {
        CustomIcon: "",
        CustomRulesetPath4Singbox: "",
        DomainStrategy: "AsIs",
        DomainStrategy4Singbox: "",
        Enabled: true,
        Id: id,
        IsActive: active,
        Locked: false,
        Remarks: remarks,
        RuleNum: 0,
        RuleSet: [],
        Sort: 0,
        Url: "",
      };
    }

    function makeAppConfig() {
      return {
        CheckUpdateItem: {
          CheckPreReleaseUpdate: false,
          SelectedCoreTypes: ["app"],
        },
        ClashUIItem: {},
        ConstItem: {
          GeoSourceUrl: null,
          RouteRulesTemplateSourceUrl: null,
          SrsSourceUrl: null,
        },
        CoreBasicItem: {},
        CoreTypeItem: [],
        Fragment4RayItem: {},
        GUIItem: {},
        GrpcItem: {},
        HysteriaItem: {},
        Inbound: [],
        IndexId: "",
        KcpItem: {},
        MsgUIItem: {},
        Mux4RayItem: {},
        Mux4SboxItem: {},
        RoutingBasicItem: {},
        SimpleDNSItem: {},
        SpeedTestItem: {},
        SubIndexId: "",
        SystemProxyItem: {
          CustomSystemProxyPacPath: null,
          CustomSystemProxyScriptPath: null,
          NotProxyLocalAddress: true,
          SysProxyType: 0,
          SystemProxyAdvancedProtocol: "",
          SystemProxyExceptions: "",
        },
        TunModeItem: {},
        UIItem: {
          ColorPrimaryName: "Teal",
          CurrentFontFamily: "",
          CurrentFontSize: 16,
          CurrentLanguage: "en",
          CurrentTheme: "FollowSystem",
        },
        WebDavItem: {
          DirName: null,
          Password: null,
          Url: null,
          UserName: null,
        },
      };
    }

    function makeDnsSettings() {
      return {
        defaults: {
          singboxNormalDns: "{\"servers\":[]}",
          singboxTunDns: "{\"servers\":[]}",
        },
        simpleDnsItem: {
          AddCommonHosts: true,
          BlockBindingQuery: false,
          BootstrapDNS: "1.1.1.1",
          DirectDNS: "223.5.5.5",
          DirectExpectedIPs: "",
          FakeIP: false,
          GlobalFakeIp: false,
          Hosts: "",
          ParallelQuery: false,
          RemoteDNS: "https://1.1.1.1/dns-query",
          ServeStale: false,
          Strategy4Freedom: "AsIs",
          Strategy4Proxy: "UseIP",
          UseSystemHosts: true,
        },
        singboxDnsItem: {
          CoreType: 24,
          DomainDNSAddress: null,
          DomainStrategy4Freedom: null,
          Enabled: false,
          Id: "dns-singbox",
          NormalDNS: "{\"servers\":[]}",
          Remarks: "sing-box",
          TunDNS: "{\"servers\":[]}",
          UseSystemHosts: false,
        },
      };
    }

    function makeHotkeyStatus() {
      const labels = ["Show window", "Clear system proxy", "Set system proxy", "Keep system proxy", "Set PAC proxy"];
      return {
        actions: labels.map((label, action) => ({ action, label })),
        registered: [],
        settings: labels.map((_label, EGlobalHotkey) => ({
          Alt: false,
          Control: false,
          EGlobalHotkey,
          KeyCode: null,
          Shift: false,
        })),
      };
    }

    function clone<T>(value: T): T {
      return value === undefined ? value : JSON.parse(JSON.stringify(value));
    }

    function cloneRecord(value: unknown) {
      return value && typeof value === "object" ? clone(value as Record<string, unknown>) : {};
    }

    function mergeDeep<T extends Record<string, unknown>>(target: T, patch: Record<string, unknown>): T {
      const next = clone(target) as Record<string, unknown>;
      Object.entries(patch).forEach(([key, value]) => {
        if (value && typeof value === "object" && !Array.isArray(value) && next[key] && typeof next[key] === "object" && !Array.isArray(next[key])) {
          next[key] = mergeDeep(next[key] as Record<string, unknown>, value as Record<string, unknown>);
        } else {
          next[key] = value;
        }
      });
      return next as T;
    }

    function readRecord(args: CommandArgs, key: string) {
      const value = args[key];
      return value && typeof value === "object" ? (value as Record<string, unknown>) : {};
    }

    function readArray(args: CommandArgs, key: string) {
      const value = args[key];
      return Array.isArray(value) ? value : [];
    }

    function readStringArray(args: CommandArgs, key: string) {
      return readArray(args, key).map(String);
    }

    function readNullableStringArray(input: Record<string, unknown>, key: string) {
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
  });
}

declare global {
  interface Window {
    __TAURI_EVENT_PLUGIN_INTERNALS__: {
      unregisterListener: () => undefined;
    };
    __TAURI_INTERNALS__: {
      invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown>;
      transformCallback: (callback: (event: { id: number; event: string; payload: unknown }) => void) => number;
      unregisterCallback: (id: number) => void;
    };
    __VOYA_SMOKE__: {
      emit: (event: string, payload: unknown) => void;
      state: unknown;
    };
  }
}
