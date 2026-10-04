import type { PolicyGroupEntry } from "@voya/contracts";
import { profileMemberName } from "@voya/features/profiles/profile-display";
import { Button } from "@voya/ui/components/button";
import { cn } from "@voya/ui/lib/utils";
import { formatDelay } from "@voya/utils/formatting";

/**
 * A group's members as chips, the one loop the Nodes cards and the Proxy page
 * share. Each caller keeps its own container — a wrapping row with a "show
 * all" control, a grid — and passes `onChoose` only under a selector strategy.
 */
export function PolicyGroupMemberChips({
  currentId,
  delayPlaceholder,
  delays,
  members,
  onChoose,
  stretch,
  testId,
}: {
  currentId: string | null | undefined;
  delayPlaceholder?: string;
  delays: ReadonlyMap<string, number | null>;
  members: PolicyGroupEntry["members"];
  onChoose?: (profileId: string) => void;
  stretch?: boolean;
  testId?: string;
}) {
  return members.map((member) => (
    <PolicyGroupMemberChip
      current={member.profileId === currentId}
      delay={formatDelay(delays.get(member.profileId))}
      delayPlaceholder={delayPlaceholder}
      key={member.profileId}
      name={profileMemberName(member.remarks, member.profileId)}
      onChoose={onChoose}
      profileId={member.profileId}
      stretch={stretch}
      testId={testId}
    />
  ));
}

/**
 * One member of a policy group, as both the Nodes cards and the Proxy page
 * render it: a live switch button under a selector strategy, a static chip
 * whose highlight marks the core's choice under an automatic one.
 *
 * `stretch` adapts the same markup to the two layouts: the Proxy page's grid
 * stretches the name and pushes the delay to the end, the Nodes cards keep
 * the chip at its content width.
 */
function PolicyGroupMemberChip({
  current,
  delay,
  delayPlaceholder,
  name,
  onChoose,
  profileId,
  stretch = false,
  testId,
}: {
  current: boolean;
  /** The formatted delay, or `null`/`undefined` when none was measured. */
  delay: string | null | undefined;
  /** Shown instead of a missing delay, e.g. the Proxy page's em dash. */
  delayPlaceholder?: string;
  name: string;
  onChoose?: (profileId: string) => void;
  profileId: string;
  stretch?: boolean;
  testId?: string;
}) {
  // `||`, not `??`: a member with no measurement formats to an empty string.
  const fallback = delay || delayPlaceholder;
  const content = (
    <>
      <span className={stretch ? "min-w-0 flex-1 truncate text-start" : "max-w-48 truncate"}>
        {name}
      </span>
      {fallback ? <span className="text-xs text-muted-foreground">{fallback}</span> : null}
    </>
  );

  return onChoose ? (
    <Button
      aria-pressed={current}
      className={stretch ? "justify-between" : undefined}
      onClick={() => onChoose(profileId)}
      size="sm"
      type="button"
      variant={current ? "secondary" : "outline"}
    >
      {content}
    </Button>
  ) : (
    <span
      className={cn(
        "inline-flex h-8 items-center gap-2 rounded-md border px-3 text-sm",
        current && "border-primary text-brand",
      )}
      data-current={current || undefined}
      data-testid={testId}
    >
      {content}
    </span>
  );
}
