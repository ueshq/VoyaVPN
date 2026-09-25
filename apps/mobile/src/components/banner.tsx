import { Alert } from "heroui-native/alert";
import type { ReactNode } from "react";

type BannerStatus = "danger" | "info" | "warning";

/** HeroUI's name for each status; its `accent` is the Voya brand blue. */
const ALERT_STATUS = {
  danger: "danger",
  info: "accent",
  warning: "warning",
} as const satisfies Record<BannerStatus, string>;

/**
 * A message about the screen it sits on — a failure, a warning, the outcome of
 * an action — as a HeroUI `Alert`, so it reads as a message rather than as
 * more of the page. `action` is at most one small button, such as a retry.
 *
 * The alert's root carries `role="alert"` but is not itself an accessibility
 * element, so the action inside stays reachable on its own.
 */
export function Banner({
  action,
  liveRegion = false,
  message,
  status,
}: {
  action?: ReactNode;
  /** Announce changes to VoiceOver/TalkBack, for outcomes that arrive later. */
  liveRegion?: boolean;
  message: string;
  status: BannerStatus;
}) {
  return (
    <Alert status={ALERT_STATUS[status]}>
      <Alert.Indicator accessible={false} />
      <Alert.Content className="min-w-0 items-start gap-2">
        <Alert.Title accessibilityLiveRegion={liveRegion ? "polite" : undefined}>{message}</Alert.Title>
        {action}
      </Alert.Content>
    </Alert>
  );
}
