import { existsSync, readFileSync } from "node:fs";
import { relative, resolve } from "node:path";
import { repoRootFromScript } from "../lib/common.mjs";
import { readJson, walkFilesSync } from "../lib/fs.mjs";
import {
  EXTERNAL_KEY_NAMESPACES,
  inspectI18nSource,
  pluralBaseKey,
  unusedTranslationKeys,
} from "./i18n-analyzer.mjs";

const repoRoot = repoRootFromScript(import.meta.url);
const localesDir = resolve(repoRoot, "packages/i18n/src/locales");
const productionSourceDirs = [
  resolve(repoRoot, "apps/desktop/src"),
  resolve(repoRoot, "packages/ui/src"),
  // The shared client owns `messages.ts`, the only place a backend code becomes
  // a translation key, so its keys would read as unused without this entry.
  resolve(repoRoot, "packages/client/src"),
  // Shared feature logic and controllers; the desktop screens that used to hold
  // these keys now import them from here.
  resolve(repoRoot, "packages/features/src"),
  // The React Native app. Its screens are the second reader of most of these
  // keys, and the only reader of a few — a key a phone alone shows would read
  // as unused without this entry.
  resolve(repoRoot, "apps/mobile/src"),
];
const localeCodes = ["en", "zh-Hans", "zh-Hant"];

const resources = Object.fromEntries(localeCodes.map((code) => [code, readLocale(code)]));
const englishKeys = flattenResourceKeys(resources.en).sort();
// What source may name: a plain leaf, or a plural message by its base. A form
// (`x_one`) is not on the list, so picking one by hand is an undefined key —
// the choice belongs to i18next and the `count` it is given.
const knownKeys = new Set(englishKeys.map((key) => pluralBaseKey(key) ?? key));

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

const pluralProblems = [];
for (const base of new Set(englishKeys.map(pluralBaseKey).filter(Boolean))) {
  // Every language has the catch-all form, whatever others it adds.
  if (!englishKeys.includes(`${base}_other`)) {
    pluralProblems.push(`${base} has no _other form`);
  }
  // Chinese has one plural form. Its `_one` exists because a runtime without
  // `Intl.PluralRules` falls back to an English-like rule and asks for it at a
  // count of one; it must read the same as `_other`, or that runtime shows a
  // different sentence than every other.
  for (const code of localeCodes.filter((locale) => locale.startsWith("zh"))) {
    const one = lookup(resources[code], `${base}_one`);
    if (one !== undefined && one !== lookup(resources[code], `${base}_other`)) {
      pluralProblems.push(`${code}: ${base}_one differs from ${base}_other`);
    }
  }
}
if (pluralProblems.length > 0) {
  throw new Error(`Plural messages are inconsistent:\n${formatList(pluralProblems)}`);
}

if (englishKeys.some((key) => key.startsWith("resx."))) {
  throw new Error("The retired resx translation namespace must not be reintroduced.");
}

const invalidKeys = [];
const dynamicKeys = [];
const hardcodedJsx = [];
const sourceLiterals = new Set();

for (const root of productionSourceDirs) {
  for (const path of productionSourceFiles(root)) {
    inspectSource(path);
  }
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
  invalidKeys.push(...result.invalidKeys.map((item) => location(path, item)));
  dynamicKeys.push(...result.dynamicKeys.map((item) => location(path, item)));
  for (const literal of result.literals) sourceLiterals.add(literal);
  hardcodedJsx.push(...result.hardcodedText.map((item) => location(path, item)));
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

function lookup(resource, key) {
  return key.split(".").reduce((node, part) => (isPlainObject(node) ? node[part] : undefined), resource);
}

function location(path, item) {
  return `${relative(repoRoot, path)}:${item.line} ${item.detail}`;
}

function formatList(items) {
  return items.map((item) => `- ${item}`).join("\n");
}

function productionSourceFiles(root) {
  return walkFilesSync(root, {
    matchDirectory: (name) => !["__tests__", "test", "tests"].includes(name),
    match: (name) =>
      /\.(ts|tsx)$/.test(name)
      && !/\.(test|spec)\.(ts|tsx)$/.test(name)
      && name !== "bindings.ts",
  });
}

function isPlainObject(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
