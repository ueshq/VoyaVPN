import { placeholderText } from "../../validation.mjs";
import { defaultTauriConfig, resolveRepoPath, readJsonAsync, displayPath, isDryRun } from "./inputs.mjs";

function isPlainObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function mergeConfig(base, overlay) {
  const merged = { ...base };
  for (const [key, value] of Object.entries(overlay)) {
    if (isPlainObject(value) && isPlainObject(merged[key])) {
      merged[key] = mergeConfig(merged[key], value);
    } else {
      merged[key] = value;
    }
  }
  return merged;
}

function isCredentialFreeUpdaterConfig(updater) {
  return (
    updater &&
    typeof updater === "object" &&
    placeholderText(updater.pubkey) &&
    Array.isArray(updater.endpoints) &&
    updater.endpoints.length === 0
  );
}

async function loadTauriConfig(options) {
  const basePath = resolveRepoPath(defaultTauriConfig);
  const requestedPath = resolveRepoPath(options.tauriConfig);
  const baseConfig = await readJsonAsync(basePath);

  if (requestedPath === basePath) {
    return {
      config: baseConfig,
      label: displayPath(basePath),
      isDefaultConfig: true,
    };
  }

  const overlay = await readJsonAsync(requestedPath);
  return {
    config: mergeConfig(baseConfig, overlay),
    label: `${displayPath(basePath)} + ${displayPath(requestedPath)}`,
    isDefaultConfig: false,
  };
}

export async function checkTauriConfig(reporter, options, updatesBaseUrl) {
  const { config, label, isDefaultConfig } = await loadTauriConfig(options);
  const updater = config.plugins?.updater;
  const bundle = config.bundle ?? {};
  const detailsPrefix = label;
  const defaultDryRunConfig = isDryRun(options) && isDefaultConfig;

  if (!updater || typeof updater !== "object") {
    if (defaultDryRunConfig) {
      reporter.pass("Tauri updater config", [
        `${detailsPrefix}: dry-run uses the credential-free base config; stable mode validates the generated updater overlay`,
      ]);
    } else {
      reporter.blocker("Tauri updater config", [`${detailsPrefix}: plugins.updater is missing`]);
    }
  } else if (defaultDryRunConfig && isCredentialFreeUpdaterConfig(updater)) {
    reporter.pass("Tauri updater config", [
      `${detailsPrefix}: dry-run uses an empty credential-free updater config; stable mode validates the generated updater overlay`,
    ]);
  } else {
    if (placeholderText(updater.pubkey)) {
      reporter.blocker("Tauri updater public key", [`${detailsPrefix}: plugins.updater.pubkey is empty or a placeholder`]);
    } else if (String(updater.pubkey).trim().length < 32) {
      reporter.blocker("Tauri updater public key", [`${detailsPrefix}: plugins.updater.pubkey is too short`]);
    } else {
      reporter.pass("Tauri updater public key", [`${detailsPrefix}: public key is non-placeholder`]);
    }

    if (!Array.isArray(updater.endpoints) || updater.endpoints.length === 0) {
      reporter.blocker("Tauri updater endpoints", [`${detailsPrefix}: plugins.updater.endpoints is missing or empty`]);
    } else {
      const updateHost = new URL(updatesBaseUrl).hostname.toLowerCase();
      const badEndpoints = [];
      for (const endpoint of updater.endpoints) {
        const endpointText = String(endpoint);
        if (placeholderText(endpointText)) {
          badEndpoints.push(`${endpointText} uses an example or placeholder URL`);
          continue;
        }

        try {
          const parsed = new URL(
            endpointText
              .replaceAll("{{target}}", "darwin")
              .replaceAll("{{arch}}", "aarch64")
              .replaceAll("{{current_version}}", "0.1.0"),
          );
          if (parsed.protocol !== "https:") {
            badEndpoints.push(`${endpointText} does not use https`);
          }
          if (options.mode === "stable" && parsed.hostname.toLowerCase() !== updateHost) {
            badEndpoints.push(`${endpointText} does not match updater base host ${updateHost}`);
          }
          if (options.mode === "stable" && !parsed.toString().startsWith(`${updatesBaseUrl}/`)) {
            badEndpoints.push(`${endpointText} is not derived from updater base URL ${updatesBaseUrl}`);
          }
        } catch {
          badEndpoints.push(`${endpointText} is not parseable as a URL template`);
        }
      }

      if (badEndpoints.length > 0) {
        reporter.blocker("Tauri updater endpoints", badEndpoints);
      } else {
        reporter.pass("Tauri updater endpoints", [`${updater.endpoints.length} endpoint template(s) use stable URL rules`]);
      }
    }
  }

  if (bundle.createUpdaterArtifacts === true) {
    reporter.pass("Tauri updater artifact flag", [`${detailsPrefix}: bundle.createUpdaterArtifacts is enabled`]);
  } else if (defaultDryRunConfig) {
    reporter.pass("Tauri updater artifact flag", [
      `${detailsPrefix}: disabled for credential-free dry runs; stable overlay must enable updater artifacts`,
    ]);
  } else {
    reporter.blocker("Tauri updater artifact flag", [
      `${detailsPrefix}: bundle.createUpdaterArtifacts is not enabled; pass a stable overlay with updater artifacts enabled`,
    ]);
  }

  const resources = bundle.resources ?? {};
  if (resources["../../../docs/release/THIRD_PARTY_NOTICES.md"] === "release/THIRD_PARTY_NOTICES.md") {
    reporter.pass("bundled notices resource", [`${detailsPrefix}: THIRD_PARTY_NOTICES.md is bundled`]);
  } else {
    reporter.fail("bundled notices resource", [`${detailsPrefix}: release notices resource is missing from bundle.resources`]);
  }
}
