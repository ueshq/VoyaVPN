import type { TranslationFunction } from "@voya/i18n/core";
import type { TunStatus } from "@voya/contracts";

/** The tunnel backend, its state and, when known, what went wrong, as one line. */
export function tunProviderLabel(tun: TunStatus, t: TranslationFunction) {
  const backend = tunBackendLabel(tun.backend, t);
  const providerState = tunProviderStateLabel(tun.providerState, t);
  const description = tunProviderErrorDescription(tun, t);
  if (description) {
    return `${backend}: ${providerState}: ${description}`;
  }

  return `${backend}: ${providerState}`;
}

export function tunProviderErrorDescription(tun: TunStatus, t: TranslationFunction) {
  if (
    tun.backend === "macosPacketTunnel" &&
    tun.providerState === "missingComponent"
  ) {
    return t("status.macosTunnelMissing");
  }

  return tun.lastProviderError;
}

export function tunProviderPathMismatchDescription(status: TunStatus, t: TranslationFunction) {
  return t("status.tunProviderPathMismatch", {
    expected: status.expectedProviderPath ?? "—",
    resolved: status.resolvedProviderPath ?? "—",
  });
}

function tunBackendLabel(backend: TunStatus["backend"], t: TranslationFunction) {
  switch (backend) {
    case "macosPacketTunnel":
      return t("status.tunBackendMacos");
    case "windowsService":
      return t("status.tunBackendWindows");
    case "process":
      return t("status.tunBackendProcess");
    case "unsupported":
    default:
      return t("status.tunBackendUnsupported");
  }
}

function tunProviderStateLabel(
  state: TunStatus["providerState"],
  t: TranslationFunction,
) {
  switch (state) {
    case "running":
      return t("status.tunProviderRunning");
    case "starting":
      return t("status.tunProviderStarting");
    case "stopped":
      return t("status.tunProviderStopped");
    case "permissionRequired":
      return t("status.tunProviderPermissionRequired");
    case "missingComponent":
      return t("status.tunProviderMissingComponent");
    case "error":
      return t("status.tunProviderError");
    case "notApplicable":
    default:
      return t("status.tunProviderNotApplicable");
  }
}
