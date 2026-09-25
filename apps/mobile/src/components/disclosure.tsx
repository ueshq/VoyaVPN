import { Accordion } from "heroui-native/accordion";
import { Typography } from "heroui-native/text";
import type { ReactNode } from "react";

const ITEM = "section";

/**
 * A section that opens and closes under its title: one HeroUI `Accordion`
 * item. The trigger is a button carrying the `expanded` state a screen reader
 * announces, and collapsed content is not mounted at all.
 *
 * The trigger and the content drop the accordion's own inline padding so the
 * title lines up with the page content around it.
 *
 * Pass `isExpanded` to control it — the DNS page keeps its fields open while
 * they hold unsaved edits. Controlled, "closed" is the empty value rather than
 * `undefined`, which the accordion would read as "uncontrolled".
 */
export function Disclosure({
  children,
  heading = false,
  isExpanded,
  onExpandedChange,
  title,
}: {
  children: ReactNode;
  /** Set the title as a section heading rather than as row text. */
  heading?: boolean;
  isExpanded?: boolean;
  onExpandedChange?: (isExpanded: boolean) => void;
  title: string;
}) {
  return (
    <Accordion
      selectionMode="single"
      hideSeparator
      value={isExpanded === undefined ? undefined : isExpanded ? ITEM : ""}
      onValueChange={(value: string | string[] | undefined) => onExpandedChange?.(value === ITEM)}
    >
      <Accordion.Item value={ITEM}>
        <Accordion.Trigger className="min-h-control px-0">
          <Typography
            maxFontSizeMultiplier={heading ? 2 : undefined}
            className={`min-w-0 flex-1 text-foreground ${heading ? "text-xl font-semibold" : "text-base"}`}
          >
            {title}
          </Typography>
          <Accordion.Indicator />
        </Accordion.Trigger>
        <Accordion.Content className="gap-4 px-0">{children}</Accordion.Content>
      </Accordion.Item>
    </Accordion>
  );
}
