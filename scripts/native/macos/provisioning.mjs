import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { basename, dirname, extname, join, resolve } from "node:path";
import { capture, checkedCapture, repoRootFromScript } from "../../lib/common.mjs";
import { requiredNetworkExtensionValue } from "./tunnel-layout.mjs";

const repoRoot = repoRootFromScript(import.meta.url);
const defaultDecodedDir = resolve(repoRoot, "target", "native", "macos", "decoded-provisioning-profiles");

export function plistBuddy(plistPath, keyPath, optional = false) {
  const result = capture("/usr/libexec/PlistBuddy", ["-c", `Print ${keyPath}`, plistPath], {
    cwd: repoRoot,
  });
  if (result.status !== 0) {
    if (optional) {
      return "";
    }
    throw new Error(`Unable to read ${keyPath} from ${plistPath}: ${result.stderr || result.stdout}`);
  }
  return result.stdout.trim();
}

function plistBuddyXml(plistPath, keyPath, optional = false) {
  const result = capture("/usr/libexec/PlistBuddy", ["-x", "-c", `Print ${keyPath}`, plistPath], {
    cwd: repoRoot,
  });
  if (result.status !== 0) {
    if (optional) {
      return "";
    }
    throw new Error(`Unable to read ${keyPath} from ${plistPath}: ${result.stderr || result.stdout}`);
  }
  return result.stdout.trim();
}

function parsePlistArray(output) {
  return output
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line && line !== "Array {" && line !== "}");
}

export function parseCodesigningIdentities(output) {
  const identities = [];
  for (const line of String(output ?? "").split(/\r?\n/)) {
    // Invalid identities in the non `-v` listing carry a trailing annotation
    // such as (CSSMERR_TP_NOT_TRUSTED); accept it so callers can detect them.
    const match = line.match(/^\s*\d+\)\s+([A-Fa-f0-9]{40})\s+"(.+)"(?:\s+\((CSSMERR_[A-Z0-9_.]+)\))?\s*$/);
    if (match) {
      identities.push({ sha1: match[1].toUpperCase(), name: match[2], error: match[3] ?? null });
    }
  }
  return identities;
}

export function findMatchingIdentities(identities, spec) {
  if (spec instanceof RegExp) {
    return identities.filter((identity) => spec.test(identity.name));
  }
  const trimmed = String(spec ?? "").trim();
  if (!trimmed) {
    return [];
  }
  return identities.filter(
    (identity) =>
      identity.sha1 === trimmed.toUpperCase() || identity.name === trimmed || identity.name.includes(trimmed),
  );
}

export function certificateSha1Fingerprint(derBuffer) {
  return createHash("sha1").update(derBuffer).digest("hex").toUpperCase();
}

export function profileMatchesDistribution(profile, distribution, allowDevelopmentProvisioning = false) {
  if (distribution === "developer-id") {
    return profile.developerCertificateSubjects.some((subject) => subject.includes("CN=Developer ID Application:"));
  }
  if (distribution === "app-store") {
    return profile.developerCertificateSubjects.some(
      (subject) =>
        subject.includes("CN=3rd Party Mac Developer Application:")
        || subject.includes("CN=Apple Distribution:")
        || (allowDevelopmentProvisioning
          && (subject.includes("CN=Apple Development:") || subject.includes("CN=Mac Developer:"))),
    );
  }
  return true;
}

export function distributionProfileLabel(distribution) {
  return distribution === "developer-id" ? "Developer ID" : "App Store/TestFlight";
}

export function profileContainsCertificate(profile, identitySha1) {
  const target = String(identitySha1 ?? "").trim().toUpperCase();
  if (!target) {
    return false;
  }
  return (profile.developerCertificateFingerprints ?? []).some(
    (fingerprint) => String(fingerprint).toUpperCase() === target,
  );
}

export function profileDeviceCoverage(profile, udid) {
  if (profile.provisionsAllDevices) {
    return "all-devices";
  }
  if (!Array.isArray(profile.provisionedDevices)) {
    return "no-device-list";
  }
  const target = String(udid ?? "").trim().toUpperCase();
  if (!target) {
    return "unknown";
  }
  return profile.provisionedDevices.some((device) => String(device).trim().toUpperCase() === target)
    ? "listed"
    : "not-listed";
}

export function profileRejectionReason(profile, criteria, now = new Date()) {
  const {
    bundleIdentifier,
    distribution,
    allowDevelopmentProvisioning = false,
    identitySha1 = null,
    deviceUdid = null,
  } = criteria;

  if (bundleIdentifier && profile.bundleIdentifier !== bundleIdentifier) {
    return `bundle id is ${profile.bundleIdentifier || "(unknown)"}, expected ${bundleIdentifier}`;
  }
  if (profile.expirationDate) {
    const expires = new Date(profile.expirationDate);
    if (!Number.isNaN(expires.getTime()) && expires.getTime() <= now.getTime()) {
      return `expired on ${profile.expirationDate}`;
    }
  }
  if (!profileMatchesDistribution(profile, distribution, allowDevelopmentProvisioning)) {
    return `has no ${distributionProfileLabel(distribution)} signing certificate`;
  }
  if (identitySha1 && !profileContainsCertificate(profile, identitySha1)) {
    return `signing certificate ${identitySha1} is not in the profile's DeveloperCertificates`;
  }
  const coverage = profileDeviceCoverage(profile, deviceUdid);
  if (coverage === "not-listed") {
    return `this Mac (UDID ${deviceUdid}) is not in the profile's ProvisionedDevices`;
  }
  if (coverage === "unknown") {
    return "the profile restricts devices but this Mac's provisioning UDID could not be determined (set VOYAVPN_PROVISIONING_UDID)";
  }
  return null;
}

export function selectProvisioningProfile(profiles, criteria, now = new Date()) {
  const rejections = [];
  for (const profile of profiles) {
    const reason = profileRejectionReason(profile, criteria, now);
    if (!reason) {
      return { profile, rejections };
    }
    rejections.push({ path: profile.path, name: profile.name, reason });
  }
  return { profile: null, rejections };
}

const requiredAppGroup = "group.app.voyavpn.desktop";

/**
 * The one profile-capability check, shared by build-tunnel, sign-app and
 * verify-tunnel.
 *
 * These three grew three hand-rolled copies and the copies diverged: sign-app
 * skipped the distribution-specific NetworkExtension value whenever the profile
 * merely contained the generic `packet-tunnel-provider`, so a Developer ID
 * profile without `packet-tunnel-provider-systemextension` passed signing and
 * `writeProfileEntitlements` then wrote the profile's entitlements verbatim
 * into a mis-entitled app.
 */
export function assertProfileCapabilities(profile, { label, distribution }) {
  if (!profile.appGroups.includes(requiredAppGroup)) {
    throw new Error(`${label} provisioning profile ${profile.path} does not include ${requiredAppGroup}.`);
  }

  const requiredValue = requiredNetworkExtensionValue(distribution);
  if (!profile.networkExtensions.includes(requiredValue)) {
    throw new Error(`${label} provisioning profile ${profile.path} does not include ${requiredValue}.`);
  }
}

export function formatProfileSelectionError(label, bundleIdentifier, rejections, profileDir) {
  const lines = [`${label} provisioning profile for ${bundleIdentifier} was not found in ${profileDir}.`];
  if (rejections.length) {
    lines.push("Profiles were considered and rejected:");
    for (const rejection of rejections) {
      lines.push(`  - ${rejection.name || basename(rejection.path)}: ${rejection.reason}`);
    }
  } else {
    lines.push("No .provisionprofile/.mobileprovision files were found there.");
  }
  lines.push(
    "Set VOYAVPN_PROVISIONING_PROFILE_DIR (or the explicit profile env vars); see docs/release/macos-local-tun-testing.md for local development profile setup.",
  );
  return lines.join("\n");
}

export function listCodesigningIdentities() {
  return parseCodesigningIdentities(checkedCapture("security", ["find-identity", "-v", "-p", "codesigning"], { cwd: repoRoot }).stdout);
}

export function resolveSigningIdentity(spec, label = "codesigning") {
  const identities = listCodesigningIdentities();
  const matches = findMatchingIdentities(identities, spec);
  if (matches.length === 1) {
    return matches[0];
  }
  const specLabel = spec instanceof RegExp ? String(spec) : `"${spec}"`;
  if (matches.length === 0) {
    const found = identities.length
      ? `Valid identities:\n${identities.map((identity) => `  ${identity.sha1} "${identity.name}"`).join("\n")}`
      : "No valid codesigning identities were found in the keychain.";
    throw new Error(`No valid ${label} signing identity matches ${specLabel}. ${found}`);
  }
  const uniqueSha1s = new Set(matches.map((identity) => identity.sha1));
  if (uniqueSha1s.size === 1) {
    return matches[0];
  }
  throw new Error(
    `Multiple ${label} signing identities match ${specLabel}; set VOYAVPN_CODESIGN_IDENTITY to one SHA-1:\n${matches
      .map((identity) => `  ${identity.sha1} "${identity.name}"`)
      .join("\n")}`,
  );
}

export function localProvisioningUdid() {
  const explicit = process.env.VOYAVPN_PROVISIONING_UDID?.trim();
  if (explicit) {
    return explicit;
  }
  const result = capture("system_profiler", ["SPHardwareDataType", "-json"], {
    cwd: repoRoot,
  });
  if (result.error || result.status !== 0) {
    return null;
  }
  try {
    const udid = JSON.parse(result.stdout)?.SPHardwareDataType?.[0]?.provisioning_UDID;
    return typeof udid === "string" && udid.trim() ? udid.trim() : null;
  } catch {
    return null;
  }
}

function collectProvisioningProfiles(root, results = []) {
  if (!existsSync(root)) {
    return results;
  }

  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) {
      collectProvisioningProfiles(path, results);
      continue;
    }
    const extension = extname(entry.name).toLowerCase();
    if (extension === ".provisionprofile" || extension === ".mobileprovision") {
      results.push(path);
    }
  }

  return results;
}

function developerCertificates(plistPath) {
  const certificates = [];
  for (let index = 0; ; index += 1) {
    const certificateXml = plistBuddyXml(plistPath, `:DeveloperCertificates:${index}`, true);
    if (!certificateXml) {
      break;
    }
    const dataMatch = certificateXml.match(/<data>\s*([\s\S]*?)\s*<\/data>/);
    if (!dataMatch) {
      continue;
    }
    const certificate = Buffer.from(dataMatch[1].replace(/\s+/g, ""), "base64");
    const subject = checkedCapture("openssl", ["x509", "-inform", "DER", "-noout", "-subject"], {
      cwd: repoRoot,
      input: certificate,
    }).stdout.trim();
    certificates.push({ subject, fingerprint: certificateSha1Fingerprint(certificate) });
  }
  return certificates;
}

function readProvisionedDevices(plistPath) {
  const xml = plistBuddyXml(plistPath, ":ProvisionedDevices", true);
  if (!xml) {
    return null;
  }
  return [...xml.matchAll(/<string>([^<]*)<\/string>/g)].map((match) => match[1].trim()).filter(Boolean);
}

function readExpirationDate(plistPath) {
  const result = capture("plutil", ["-extract", "ExpirationDate", "raw", "-o", "-", plistPath], {
    cwd: repoRoot,
  });
  if (result.error || result.status !== 0) {
    return null;
  }
  const value = result.stdout.trim();
  return value || null;
}

export function decodeProvisioningProfile(profilePath, decodedDir = defaultDecodedDir) {
  const decoded = checkedCapture("security", ["cms", "-D", "-i", profilePath], { cwd: repoRoot }).stdout;
  mkdirSync(decodedDir, { recursive: true });
  const plistPath = resolve(decodedDir, `${basename(profilePath)}.plist`);
  writeFileSync(plistPath, decoded);

  const applicationIdentifier = plistBuddy(plistPath, ":Entitlements:com.apple.application-identifier");
  const teamIdentifier = plistBuddy(plistPath, ":Entitlements:com.apple.developer.team-identifier", true)
    || plistBuddy(plistPath, ":TeamIdentifier:0", true);
  const bundleIdentifier = teamIdentifier && applicationIdentifier.startsWith(`${teamIdentifier}.`)
    ? applicationIdentifier.slice(teamIdentifier.length + 1)
    : applicationIdentifier.replace(/^[^.]+\./, "");
  const certificates = developerCertificates(plistPath);

  return {
    path: profilePath,
    name: plistBuddy(plistPath, ":Name", true),
    uuid: plistBuddy(plistPath, ":UUID", true),
    applicationIdentifier,
    bundleIdentifier,
    teamIdentifier,
    appGroups: parsePlistArray(plistBuddy(plistPath, ":Entitlements:com.apple.security.application-groups", true)),
    appSandbox: plistBuddy(plistPath, ":Entitlements:com.apple.security.app-sandbox", true) === "true",
    developerCertificateSubjects: certificates.map((certificate) => certificate.subject),
    developerCertificateFingerprints: certificates.map((certificate) => certificate.fingerprint),
    keychainAccessGroups: parsePlistArray(plistBuddy(plistPath, ":Entitlements:keychain-access-groups", true)),
    networkClient: plistBuddy(plistPath, ":Entitlements:com.apple.security.network.client", true) === "true",
    networkServer: plistBuddy(plistPath, ":Entitlements:com.apple.security.network.server", true) === "true",
    networkExtensions: parsePlistArray(
      plistBuddy(plistPath, ":Entitlements:com.apple.developer.networking.networkextension", true),
    ),
    systemExtensionInstall: plistBuddy(
      plistPath,
      ":Entitlements:com.apple.developer.system-extension.install",
      true,
    ) === "true",
    provisionsAllDevices: plistBuddy(plistPath, ":ProvisionsAllDevices", true) === "true",
    provisionedDevices: readProvisionedDevices(plistPath),
    expirationDate: readExpirationDate(plistPath),
  };
}

export function resolveProfileFromEnv({ bundleIdentifier, explicitEnvName, profileDir, criteria, decodedDir }) {
  const fullCriteria = { ...criteria, bundleIdentifier };
  const explicit = process.env[explicitEnvName]?.trim();
  if (explicit) {
    const profilePath = resolve(explicit);
    if (!existsSync(profilePath)) {
      throw new Error(`${explicitEnvName} points to a missing file: ${profilePath}`);
    }
    const profile = decodeProvisioningProfile(profilePath, decodedDir);
    const reason = profileRejectionReason(profile, fullCriteria);
    if (reason) {
      throw new Error(`${explicitEnvName} profile ${profilePath} cannot be used: ${reason}.`);
    }
    return { profile, rejections: [] };
  }

  const profiles = collectProvisioningProfiles(profileDir).map((profilePath) =>
    decodeProvisioningProfile(profilePath, decodedDir),
  );
  return selectProvisioningProfile(profiles, fullCriteria);
}

function escapeXml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&apos;");
}

function plistStringArray(values) {
  return `<array>\n${values.map((value) => `    <string>${escapeXml(value)}</string>`).join("\n")}\n  </array>`;
}

function baseEntitlementEnabled(baseEntitlements, keyPath) {
  return plistBuddy(baseEntitlements, keyPath, true) === "true";
}

export function writeProfileEntitlements(profile, destination, baseEntitlements) {
  mkdirSync(dirname(destination), { recursive: true });
  const keychainAccessGroups = profile.keychainAccessGroups.length
    ? profile.keychainAccessGroups
    : [`${profile.teamIdentifier}.*`];
  const appGroups = profile.appGroups.length ? profile.appGroups : ["group.app.voyavpn.desktop"];
  const appSandbox = profile.appSandbox || baseEntitlementEnabled(baseEntitlements, ":com.apple.security.app-sandbox");
  const networkClient = profile.networkClient || baseEntitlementEnabled(baseEntitlements, ":com.apple.security.network.client");
  const networkServer = profile.networkServer || baseEntitlementEnabled(baseEntitlements, ":com.apple.security.network.server");
  const systemExtensionInstall =
    profile.systemExtensionInstall ||
    baseEntitlementEnabled(baseEntitlements, ":com.apple.developer.system-extension.install");
  const xml = `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "https://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>com.apple.application-identifier</key>
  <string>${escapeXml(profile.applicationIdentifier)}</string>
  <key>com.apple.developer.networking.networkextension</key>
  ${plistStringArray(profile.networkExtensions)}
  ${systemExtensionInstall ? "<key>com.apple.developer.system-extension.install</key>\n  <true/>" : ""}
  <key>com.apple.developer.team-identifier</key>
  <string>${escapeXml(profile.teamIdentifier)}</string>
  ${appSandbox ? "<key>com.apple.security.app-sandbox</key>\n  <true/>" : ""}
  <key>com.apple.security.application-groups</key>
  ${plistStringArray(appGroups)}
  ${networkClient ? "<key>com.apple.security.network.client</key>\n  <true/>" : ""}
  ${networkServer ? "<key>com.apple.security.network.server</key>\n  <true/>" : ""}
  <key>keychain-access-groups</key>
  ${plistStringArray(keychainAccessGroups)}
</dict>
</plist>
`;
  writeFileSync(destination, xml);
  return destination;
}
