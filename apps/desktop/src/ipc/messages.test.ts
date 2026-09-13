import { afterAll, describe, expect, it } from "vitest";

import { changeLocale, i18next, localeOptions, type Locale, type TranslationFunction } from "@voya/i18n";

import type { ImportLineIssue, LogCode, NoticeCode, SpeedtestOutcome, ValidationCode } from "@/ipc/bindings";
import {
  CORE_FLOW_REASON_KEYS,
  IMPORT_LINE_KEYS,
  LOG_KEYS,
  NOTICE_KEYS,
  SPEEDTEST_OUTCOME_KEYS,
  VALIDATION_KEYS,
  VALIDATION_SCOPE_KEYS,
  importLineText,
  logLineText,
  noticeText,
  speedtestOutcomeText,
  validationText,
} from "@/ipc/messages";

/**
 * The backend hands the frontend codes, not sentences. These tests are the
 * other half of that contract: `tsc` proves every code has a locale key, and
 * this file proves every key actually resolves to text in every shipped locale
 * — including a non-English one, which is the whole point of the change.
 */
function translator(locale: Locale): TranslationFunction {
  const fixedT = i18next.getFixedT(locale);

  return (key, options) => String(fixedT(key, options));
}

const en = translator("en");
const zh = translator("zh-Hans");

afterAll(async () => {
  await changeLocale("en");
});

describe("backend message codes", () => {
  it("resolves every registry entry to real text in all shipped locales", () => {
    const keys = [
      ...Object.values(NOTICE_KEYS),
      ...Object.values(LOG_KEYS),
      ...Object.values(CORE_FLOW_REASON_KEYS),
      ...Object.values(VALIDATION_KEYS),
      ...Object.values(VALIDATION_SCOPE_KEYS),
      ...Object.values(SPEEDTEST_OUTCOME_KEYS),
      ...Object.values(IMPORT_LINE_KEYS),
    ];

    expect(keys.length).toBeGreaterThan(0);
    for (const { code: locale } of localeOptions) {
      const t = translator(locale);
      for (const key of keys) {
        const text = t(key);
        expect(text.trim(), `${locale}:${key}`).not.toBe("");
        // i18next echoes the key back when it is missing.
        expect(text, `${locale}:${key}`).not.toBe(key);
      }
    }
  });

  it("renders every import line code with its line number", () => {
    const issues: ImportLineIssue[] = [
      { line: 2, code: { code: "subscriptionSourceAdded" } },
      { line: 3, code: { code: "unsupportedTransport", transport: "xhttp" } },
      { line: 4, code: { code: "parseFailed", detail: "invalid vless URI" } },
      { line: 5, code: { code: "unsupportedProtocol" } },
      { line: 6, code: { code: "missingField", protocol: "vless", field: "id" } },
      { line: 7, code: { code: "invalidPort", protocol: "trojan", port: "99999" } },
    ];
    expect(issues.map((issue) => importLineText(en, issue))).toEqual([
      "Line 2 was added as a subscription; it is being updated to import its nodes.",
      "Line 3 was skipped: the xhttp transport is not supported. Ask your provider for a WebSocket or gRPC node.",
      "Line 4 was skipped: the link is not in a format VoyaVPN can read.",
      "Line 5 was skipped: this kind of link is not supported.",
      "Line 6 was skipped: the vless link is missing id.",
      "Line 7 was skipped: 99999 is not a valid port in the trojan link.",
    ]);
    // The untranslated diagnostic never reaches the reader.
    expect(importLineText(zh, issues[2]!)).toBe("第 4 行已跳过：链接格式无法识别。");
    expect(importLineText(zh, issues[1]!)).toBe(
      "第 3 行已跳过：不支持 xhttp 传输方式，请向服务商换用 WebSocket、gRPC 等传输的节点。",
    );
  });

  it("renders every notice code, and translates it outside English", () => {
    for (const code of Object.keys(NOTICE_KEYS) as Array<NoticeCode["code"]>) {
      const notice = { code, remarks: "Feed" } as NoticeCode;
      expect(noticeText(en, notice).trim()).not.toBe("");
      expect(noticeText(zh, notice).trim()).not.toBe("");
    }

    expect(noticeText(en, { code: "trayRefreshFailed" })).toBe("Tray refresh failed");
    // The point of the whole change: a zh-Hans user reads this in Chinese.
    const chinese = noticeText(zh, { code: "trayRefreshFailed" });
    expect(chinese).not.toBe("Tray refresh failed");
    expect(chinese).toMatch(/\p{Script=Han}/u);
  });

  it("interpolates a notice parameter", () => {
    expect(
      noticeText(en, { code: "subscriptionAutoUpdateFailed", remarks: "Nightly feed" }),
    ).toContain("Nightly feed");
    expect(
      noticeText(zh, { code: "subscriptionAutoUpdateFailed", remarks: "Nightly feed" }),
    ).toContain("Nightly feed");
  });

  it("renders every log code, and only translates app-authored lines", () => {
    for (const code of Object.keys(LOG_KEYS) as Array<LogCode["code"]>) {
      const logCode = {
        attempt: 1,
        code,
        delayMs: 1500,
        imported: 3,
        reason: "routingChanged",
        remarks: "Feed",
      } as LogCode;
      expect(logLineText(en, { code: logCode, detail: null, source: "app" }).trim()).not.toBe("");
      expect(logLineText(zh, { code: logCode, detail: null, source: "app" }).trim()).not.toBe("");
    }

    // Core output and `tracing` diagnostics are the writer's own words.
    expect(logLineText(zh, { line: "inbound/mixed", source: "core" })).toBe("inbound/mixed");
    expect(logLineText(zh, { line: "voyavpn::runtime: boom", source: "diagnostic" })).toBe(
      "voyavpn::runtime: boom",
    );
  });

  it("interpolates a log reason and appends the untranslated detail", () => {
    expect(
      logLineText(en, {
        code: { code: "restartingAfterChange", reason: "dnsChanged" },
        detail: null,
        source: "app",
      }),
    ).toBe("DNS change — restarting the core");
    expect(
      logLineText(en, {
        code: { code: "coreExitRetryScheduled", attempt: 2, delayMs: 1500 },
        detail: "Core process 7 exited",
        source: "app",
      }),
    ).toBe("The core exited; retrying in 1500 ms (attempt 2): Core process 7 exited");
  });

  it("renders every validation code, and translates it outside English", () => {
    for (const code of Object.keys(VALIDATION_KEYS) as Array<ValidationCode["code"]>) {
      const validationCode = {
        child: "Leaf",
        code,
        expected: 1,
        found: 2,
        group: "Group",
        line: 3,
        max: 65535,
        message: "raw diagnostic",
        min: 576,
        minimumSeconds: 5,
        network: "kcp",
        outbound: "Node",
        path: ["a", "b"],
        pattern: "^(HK",
        port: "0",
        profileId: "leaf-a",
        protocol: "SOCKS",
        rule: "Rule",
      } as ValidationCode;
      const issue = { code: validationCode, field: "children", scope: [] };
      expect(validationText(en, issue).trim(), code).not.toBe("");
      expect(validationText(zh, issue).trim(), code).not.toBe("");
    }

    expect(validationText(en, { code: { code: "invalidPort" }, field: "port", scope: [] })).toBe(
      "The port must be between 1 and 65535",
    );
    const chinese = validationText(zh, { code: { code: "invalidPort" }, field: "port", scope: [] });
    expect(chinese).not.toBe("The port must be between 1 and 65535");
    expect(chinese).toMatch(/\p{Script=Han}/u);
  });

  it("interpolates validation parameters", () => {
    expect(
      validationText(en, {
        code: { code: "unsupportedProtocolNetwork", network: "grpc", protocol: "SOCKS" },
        field: "transport",
        scope: [],
      }),
    ).toBe("sing-box does not support SOCKS over grpc");
  });

  it("prefixes a finding with the breadcrumb the validator walked", () => {
    expect(
      validationText(en, {
        code: { code: "invalidPort" },
        field: "activeProfile",
        scope: [
          { outbound: "Node", rule: "Rule", kind: "routingRuleOutbound" },
        ],
      }),
    ).toBe("Rule Rule → Node: The port must be between 1 and 65535");
    expect(
      validationText(en, {
        code: { code: "invalidFlow" },
        field: "activeProfile",
        scope: [{ kind: "routingRuleOutbound", outbound: "Node", rule: "Ads" }],
      }),
    ).toBe("Rule Ads → Node: The flow value is not supported");
  });

  it("shows an untranslated rejection's own diagnostic verbatim", () => {
    // The deliberate escape hatch for managers that have no code yet: the text
    // is the backend's, so it must not go through a locale key.
    const text = validationText(zh, {
      code: { code: "untranslated", message: "certificate is not valid PEM" },
      field: "pem",
      scope: [],
    });

    expect(text).toBe("certificate is not valid PEM");
  });

  it("renders every speedtest outcome, and translates it outside English", () => {
    for (const outcome of Object.keys(SPEEDTEST_OUTCOME_KEYS) as SpeedtestOutcome[]) {
      expect(speedtestOutcomeText(en, outcome).trim(), outcome).not.toBe("");
      expect(speedtestOutcomeText(zh, outcome).trim(), outcome).not.toBe("");
    }

    expect(speedtestOutcomeText(en, "timedOut")).toBe("Request timed out");
    const chinese = speedtestOutcomeText(zh, "timedOut");
    expect(chinese).not.toBe("Request timed out");
    expect(chinese).toMatch(/\p{Script=Han}/u);
  });
});
