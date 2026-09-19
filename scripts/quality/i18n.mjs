import { existsSync, readdirSync, readFileSync } from "node:fs";
import { relative, resolve } from "node:path";
import { readJson, repoRootFromScript } from "../lib/common.mjs";
import {
  EXTERNAL_KEY_NAMESPACES,
  inspectI18nSource,
  isKnownHardcodedText,
  KNOWN_HARDCODED_TEXT,
  unusedTranslationKeys,
} from "./i18n-analyzer.mjs";

const repoRoot = repoRootFromScript(import.meta.url);
const localesDir = resolve(repoRoot, "packages/i18n/src/locales");
const productionSourceDirs = [
  resolve(repoRoot, "apps/desktop/src"),
  resolve(repoRoot, "packages/ui/src"),
];
const localeCodes = ["en", "zh-Hans", "zh-Hant"];

const resources = Object.fromEntries(localeCodes.map((code) => [code, readLocale(code)]));
const englishKeys = flattenResourceKeys(resources.en).sort();
const knownKeys = new Set(englishKeys);

for (const code of localeCodes) {
  const keys = flattenResourceKeys(resources[code]).sort();
  if (keys.length !== englishKeys.length || keys.some((key, index) => key !== englishKeys[index])) {
    const missing = englishKeys.filter((key) => !keys.includes(key));
    const extra = keys.filter((key) => !englishKeys.includes(key));
    throw new Error(
      `Locale ${code} is not aligned with en (missing: ${missing.join(", ") || "none"}; extra: ${extra.join(", ") || "none"}).`,
    );
  }
}

if (englishKeys.some((key) => key.startsWith("resx."))) {
  throw new Error("The retired resx translation namespace must not be reintroduced.");
}

const invalidKeys = [];
const dynamicKeys = [];
const hardcodedJsx = [];
const usedHardcodedAllowlistEntries = new Set();
const sourceLiterals = new Set();

for (const root of productionSourceDirs) {
  for (const path of productionSourceFiles(root)) {
    inspectSource(path);
  }
}

for (const entry of KNOWN_HARDCODED_TEXT) {
  if (usedHardcodedAllowlistEntries.has(`${entry.path}:${entry.text}`)) continue;
  // Reported, not failed: the owning feature may add the locale key at any
  // time and this gate must not turn red when it does.
  console.warn(`Stale hardcoded-text allowlist entry (delete it from i18n-analyzer.mjs): ${entry.path} — ${entry.text}`);
}

if (invalidKeys.length > 0) {
  throw new Error(`Production source references undefined translation keys:\n${formatList(invalidKeys)}`);
}
if (dynamicKeys.length > 0) {
  throw new Error(`Dynamic translation keys must use explicit translated values:\n${formatList(dynamicKeys)}`);
}
if (hardcodedJsx.length > 0) {
  throw new Error(`User-visible frontend text must use Voya locale resources:\n${formatList(hardcodedJsx)}`);
}

const staleNamespaces = EXTERNAL_KEY_NAMESPACES.filter(({ reader, marker }) => {
  const path = resolve(repoRoot, reader);
  return !existsSync(path) || !readFileSync(path, "utf8").includes(marker);
});
if (staleNamespaces.length > 0) {
  throw new Error(
    `External locale readers no longer read their namespace (update EXTERNAL_KEY_NAMESPACES in i18n-analyzer.mjs):\n${formatList(
      staleNamespaces.map(({ prefix, reader }) => `${prefix}* in ${reader}`),
    )}`,
  );
}
const unusedKeys = unusedTranslationKeys({
  keys: englishKeys,
  literals: sourceLiterals,
  externalPrefixes: EXTERNAL_KEY_NAMESPACES.map(({ prefix }) => prefix),
});
if (unusedKeys.length > 0) {
  throw new Error(`Locale keys no production source uses (delete them from every locale):\n${formatList(unusedKeys)}`);
}

console.log(`i18n check passed: ${localeCodes.length} aligned Voya locales, ${englishKeys.length} keys.`);

function inspectSource(path) {
  const source = readFileSync(path, "utf8");
  const result = inspectI18nSource({ path, source, knownKeys });
  // Forward slashes so the allowlist keys match on Windows too.
  const relativePath = relative(repoRoot, path).replaceAll("\\", "/");
  invalidKeys.push(...result.invalidKeys.map((item) => location(path, item)));
  dynamicKeys.push(...result.dynamicKeys.map((item) => location(path, item)));
  for (const literal of result.literals) sourceLiterals.add(literal);
  for (const item of result.hardcodedText) {
    if (isKnownHardcodedText(relativePath, item.detail)) {
      usedHardcodedAllowlistEntries.add(`${relativePath}:${item.detail}`);
      continue;
    }
    hardcodedJsx.push(location(path, item));
  }
}

function readLocale(code) {
  const path = resolve(localesDir, `${code}.json`);
  if (!existsSync(path)) {
    throw new Error(`Missing directly maintained Voya locale: ${relative(repoRoot, path)}`);
  }
  const resource = readJson(path);
  if (!isPlainObject(resource)) {
    throw new Error(`Locale root must be an object: ${relative(repoRoot, path)}`);
  }
  return resource;
}

function flattenResourceKeys(resource, prefix = "") {
  return Object.entries(resource).flatMap(([key, value]) => {
    const path = prefix ? `${prefix}.${key}` : key;
    if (isPlainObject(value)) {
      return flattenResourceKeys(value, path);
    }
    if (typeof value !== "string" || value.trim().length === 0) {
      throw new Error(`Locale value must be a non-empty string: ${path}`);
    }
    return [path];
  });
}

function location(path, item) {
  return `${relative(repoRoot, path)}:${item.line} ${item.detail}`;
}

function formatList(items) {
  return items.map((item) => `- ${item}`).join("\n");
}

function productionSourceFiles(root) {
  const files = [];
  function visit(dir) {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (entry.isDirectory() && ["__tests__", "test", "tests"].includes(entry.name)) {
        continue;
      }
      const path = resolve(dir, entry.name);
      if (entry.isDirectory()) {
        visit(path);
      } else if (
        entry.isFile()
        && /\.(ts|tsx)$/.test(entry.name)
        && !/\.(test|spec)\.(ts|tsx)$/.test(entry.name)
        && entry.name !== "bindings.ts"
      ) {
        files.push(path);
      }
    }
  }
  visit(root);
  return files;
}

function isPlainObject(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
