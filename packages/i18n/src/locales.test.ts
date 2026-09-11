import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import ts from "typescript";

import { changeLocale, getInitialLocale, i18next, localeOptions, type Locale } from "./index";

type LocaleTree = {
  [key: string]: LocaleTree | string;
};

const desktopSourceRoot = resolve(findRepoRoot(process.cwd()), "apps/desktop/src");
const sourceModules = readSourceModules(desktopSourceRoot);

describe("i18n locales", () => {
  afterEach(async () => {
    vi.restoreAllMocks();
    window.localStorage.removeItem("voyavpn.locale");
    await changeLocale("en", { persist: false });
  });

  it("registers the full product locale set", () => {
    expect(localeOptions.map((locale) => locale.code)).toEqual([
      "en",
      "zh-Hans",
      "zh-Hant",
    ]);
  });

  it.each(["de", "fa", "fr", "hu", "ru"])("ignores removed %s preferences and browser languages", (locale) => {
    window.localStorage.setItem("voyavpn.locale", locale);
    const languages = vi.spyOn(navigator, "languages", "get").mockReturnValue([locale]);
    expect(getInitialLocale()).toBe("en");

    languages.mockReturnValue([locale, "zh-TW"]);
    expect(getInitialLocale()).toBe("zh-Hant");
  });

  it("keeps every locale aligned to the English key set", () => {
    const englishKeys = flattenKeys(localeTree("en")).sort();

    for (const locale of localeOptions) {
      const localeKeys = flattenKeys(localeTree(locale.code)).sort();
      expect(localeKeys).toEqual(englishKeys);

      for (const key of localeKeys) {
        expect(getValue(localeTree(locale.code), key)).not.toBe("");
      }
    }
  });

  it("uses directly maintained Voya resources without compatibility namespaces", () => {
    const productionKeys = collectProductionTranslationKeys();

    expect(localeTree("en")).not.toHaveProperty("resx");
    expect([...productionKeys].some((key) => key.startsWith("resx."))).toBe(false);

    for (const locale of localeOptions) {
      const localeKeys = new Set(flattenKeys(localeTree(locale.code)));
      for (const key of productionKeys) {
        expect(localeKeys.has(key), `${locale.code}:${key}`).toBe(true);
      }
    }
  });

  it("translates representative UI strings in Chinese locales (no English leakage)", () => {
    // Representative keys across the modern UI namespaces and the profiles
    // sub-domain that previously leaked English values into zh-Hans/zh-Hant.
    const translatedKeys = [
      "actions.connect",
      "actions.settings",
      "nodeGroups.local",
      "nodeGroups.test",
      "subscriptions.edit",
      "confirm.deleteProfilesTitle",
      "modal.language",
      "modal.theme",
      "options.autostart",
      "panes.profiles.title",
      "panes.profiles.fields.flow",
      "panes.profiles.fields.host",
      "panes.subscriptions.empty",
      "status.connected",
      "tabs.profiles",
      "updates.title",
    ];
    const hasCjk = /[一-鿿]/;

    for (const locale of ["zh-Hans", "zh-Hant"] as const) {
      const tree = localeTree(locale);
      const english = localeTree("en");

      for (const key of translatedKeys) {
        const value = getValue(tree, key);

        expect(typeof value, `${locale}:${key}`).toBe("string");
        expect(value, `${locale}:${key}`).toMatch(hasCjk);
        expect(value, `${locale}:${key}`).not.toBe(getValue(english, key));
      }
    }
  });

  it("covers translation keys used by app source", () => {
    const staticKeys = collectProductionTranslationKeys();

    expect(staticKeys.size).toBeGreaterThan(0);

    for (const locale of localeOptions) {
      const tree = localeTree(locale.code);

      for (const key of staticKeys) {
        expect(getValue(tree, key), `${locale.code}:${key}`).toBeTruthy();
      }
    }
  });

  it("applies document metadata when language changes", async () => {
    await changeLocale("zh-Hans");
    expect(document.documentElement.lang).toBe("zh-Hans");
    expect(document.documentElement.dir).toBe("ltr");

    await changeLocale("zh-Hant");
    expect(document.documentElement.lang).toBe("zh-Hant");
    expect(document.documentElement.dir).toBe("ltr");
  });
});

function localeTree(locale: Locale) {
  return i18next.getResourceBundle(locale, "translation") as LocaleTree;
}

function flattenKeys(tree: LocaleTree, prefix = ""): string[] {
  return Object.entries(tree).flatMap(([key, value]) => {
    const path = prefix ? `${prefix}.${key}` : key;

    return typeof value === "string" ? [path] : flattenKeys(value, path);
  });
}

function getValue(tree: LocaleTree, path: string) {
  let current: LocaleTree | string | undefined = tree;

  for (const segment of path.split(".")) {
    if (!isLocaleTree(current)) {
      return undefined;
    }

    current = current[segment];
  }

  return current;
}

function isLocaleTree(value: LocaleTree | string | undefined): value is LocaleTree {
  return typeof value === "object" && value !== null;
}

function collectProductionTranslationKeys() {
  const knownKeys = new Set(flattenKeys(localeTree("en")));
  const keys = new Set<string>();

  for (const [path, source] of sourceModules) {
    const sourceFile = ts.createSourceFile(
      path,
      source,
      ts.ScriptTarget.Latest,
      true,
      path.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
    );
    function visit(node: ts.Node) {
      if ((ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) && knownKeys.has(node.text)) {
        keys.add(node.text);
      }
      ts.forEachChild(node, visit);
    }
    visit(sourceFile);
  }

  return keys;
}

function readSourceModules(sourceRoot: string): Array<[string, string]> {
  const modules: Array<[string, string]> = [];

  visitSourceDir(sourceRoot, modules, sourceRoot);

  return modules;
}

function visitSourceDir(sourceRoot: string, modules: Array<[string, string]>, dir: string) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = resolve(dir, entry.name);

    if (entry.isDirectory()) {
      visitSourceDir(sourceRoot, modules, path);
    } else if (
      entry.isFile()
      && /\.(ts|tsx)$/.test(entry.name)
      && !/\.(test|spec)\.(ts|tsx)$/.test(entry.name)
      && entry.name !== "bindings.ts"
    ) {
      modules.push([relative(sourceRoot, path).replaceAll("\\", "/"), readFileSync(path, "utf8")]);
    }
  }
}

function findRepoRoot(start: string) {
  let dir = resolve(start);

  while (true) {
    if (existsSync(resolve(dir, "pnpm-workspace.yaml"))) {
      return dir;
    }

    const parent = dirname(dir);

    if (parent === dir) {
      throw new Error("Unable to locate pnpm-workspace.yaml");
    }

    dir = parent;
  }
}
