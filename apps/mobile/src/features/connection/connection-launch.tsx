import { connectionLinkAction } from "./connection-link";
import { useQuery } from "@tanstack/react-query";
import { queries } from "@voya/client/queries";
import { runRuntimeAction, useRuntimeBusy } from "@voya/client/runtime-action";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import { useI18n } from "@voya/i18n/use-i18n";
import { useEffect, useRef, useState } from "react";
import { Linking } from "react-native";
import { navigateToTab, openPage } from "~/app/navigation";
import { useConnectionPreferences } from "@voya/client/connection-preferences";

/** Mounted once after the privacy notice and navigator are ready, never on a tab. */
export function ConnectionLaunch() {
  const { t } = useI18n();
  const core = useRuntimeEventStore((s) => s.coreState);
  const busy = useRuntimeBusy();
  const profiles = useQuery(queries.profileList);
  const groups = useQuery(queries.policyGroups);
  const [autoAtLaunch] = useState(() => useConnectionPreferences.getState().autoConnect);
  const startupHandled = useRef(false);
  const handledRequest = useRef<object | null>(null);
  const [request, setRequest] = useState<{ action: "connect" | "disconnect" | null } | null>(null);

  useEffect(() => {
    let mounted = true;
    let receivedEvent = false;
    const listener = Linking.addEventListener("url", ({ url }) => {
      const action = connectionLinkAction(url);
      if (action) {
        receivedEvent = true;
        setRequest({ action });
      }
    });
    void Linking.getInitialURL().then(
      (url) => {
        if (mounted && !receivedEvent) setRequest({ action: connectionLinkAction(url) });
      },
      () => {
        // Unknown launch intent: do not risk connecting over a requested disconnect.
        startupHandled.current = true;
        if (mounted && !receivedEvent) setRequest({ action: null });
      },
    );
    return () => {
      mounted = false;
      listener.remove();
    };
  }, []);

  useEffect(() => {
    if (core?.state === "connected" && core.activeProfileId) {
      useConnectionPreferences.getState().rememberConnected(core.activeProfileId);
    }
  }, [core?.state, core?.activeProfileId]);

  useEffect(() => {
    if (busy) startupHandled.current = true;
  }, [busy]);

  useEffect(() => {
    if (!request || !core) return;
    if (request.action !== "disconnect" && (profiles.isPending || groups.isPending)) return;
    const explicit = request.action !== null && handledRequest.current !== request;
    if (!explicit && startupHandled.current) return;
    // A user action during startup wins over auto-connect, even if it disconnects.
    if (busy) {
      if (!explicit) startupHandled.current = true;
      return;
    }
    startupHandled.current = true;
    handledRequest.current = request;
    const action = explicit
      ? request.action
      : autoAtLaunch && useConnectionPreferences.getState().autoConnect
        ? "connect"
        : null;
    if (!action) return;
    if (explicit) navigateToTab("home");
    if (action === "disconnect") {
      if (core.state === "connected" || core.state === "cleanupPending")
        void runRuntimeAction("disconnect", t, { inline: true });
      return;
    }
    if (core.state !== "disconnected" || profiles.isError || groups.isError) return;
    const selected =
      profiles.data?.entries.some((entry) => entry.isActive) || groups.data?.entries.some((entry) => entry.isActive);
    if (!selected) {
      if (explicit) openPage(profiles.data?.entries.length ? "nodePicker" : "import");
      return;
    }
    void runRuntimeAction("connect", t, { inline: true });
  }, [
    request,
    core,
    busy,
    profiles.isPending,
    groups.isPending,
    profiles.isError,
    groups.isError,
    profiles.data,
    groups.data,
    autoAtLaunch,
    t,
  ]);
  return null;
}
