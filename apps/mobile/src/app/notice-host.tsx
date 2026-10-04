import { useToastStore } from "@voya/client/toast-store";
import { useAppVisible } from "@voya/client/use-app-visible";
import { useEffect } from "react";
import { Alert } from "react-native";

/**
 * Shows the notices the shared client raises that no screen shows itself.
 *
 * `@voya/client` reports through one store: the backend's own notices (the
 * core stopped, the tunnel was taken down), a node or group switch that
 * failed, a mutation that rejected, an autosave that did not save. The desktop
 * draws that store as toasts. A phone has no toast surface, and until this
 * existed those reports reached nobody — a failed traffic-mode change simply
 * did nothing.
 *
 * So an error or a warning becomes one system alert, which is how a phone says
 * something went wrong outside the control the user is looking at. Anything
 * milder is dropped: an alert for "copied" would be worse than silence.
 * Either way the entry leaves the store, so each is handled exactly once.
 *
 * Nothing is taken off the store while the app is in the background. The
 * tunnel keeps this code running there, and an alert raised with nothing on
 * screen to show it is simply lost — "the core stopped" among them. The
 * notice waits, and is shown when the app comes back.
 */
export function NoticeHost() {
  const next = useToastStore((state) => state.toasts[0]);
  const visible = useAppVisible();

  useEffect(() => {
    if (!next || !visible) return;
    if (next.severity === "error" || next.severity === "warning") {
      Alert.alert(next.title, next.description);
    }
    useToastStore.getState().dismissToast(next.id);
  }, [next, visible]);

  return null;
}
