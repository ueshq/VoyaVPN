import { useState } from "react";
import type * as React from "react";
import { ChevronDown } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@voya/ui/components/card";
import { TextField } from "@voya/ui/components/form-fields";
import type { ProfileDraft } from "@voya/features/profiles/profile-draft";

/** What every editor panel gets: the draft, its translated errors by field, and the one way to change it. */
export type ProfilePanelProps = {
  draft: ProfileDraft;
  errors: Partial<Record<string, string>>;
  onChange: <Key extends keyof ProfileDraft>(key: Key, value: ProfileDraft[Key]) => void;
};

/** The draft fields a plain text input edits. */
type DraftTextKey = {
  [Key in keyof ProfileDraft]: string extends ProfileDraft[Key]
    ? ProfileDraft[Key] extends string
      ? Key
      : never
    : never;
}[keyof ProfileDraft];

/** A text input bound to one draft field: its value, its edits and its error. */
export function DraftTextField({
  draft,
  errors,
  name,
  onChange,
  ...input
}: ProfilePanelProps & {
  inputMode?: "numeric";
  label: string;
  name: DraftTextKey;
  placeholder?: string;
}) {
  return (
    <TextField
      {...input}
      error={errors[name]}
      onChange={(value) => onChange(name, value)}
      value={draft[name]}
    />
  );
}

export function Panel({
  children,
  collapsible = false,
  defaultOpen = true,
  icon: Icon,
  title,
}: {
  children: React.ReactNode;
  /** A collapsible panel can start closed while it has nothing worth showing. */
  collapsible?: boolean;
  defaultOpen?: boolean;
  /** Names what the section holds, so the editor's sections tell apart at a glance. */
  icon: LucideIcon;
  title: string;
}) {
  if (collapsible) {
    return (
      <CollapsiblePanel defaultOpen={defaultOpen} icon={Icon} title={title}>
        {children}
      </CollapsiblePanel>
    );
  }
  return (
    <Card className="gap-3 rounded-xl bg-surface-raised p-3 shadow-raised">
      <CardHeader className="p-0">
        <CardTitle className="flex items-center gap-2 text-section font-semibold text-foreground">
          <Icon className="size-4 text-muted-foreground" aria-hidden="true" />
          {title}
        </CardTitle>
      </CardHeader>
      <CardContent className="p-0">{children}</CardContent>
    </Card>
  );
}

// A native <details>, so the form's error handler that opens every section
// also reaches this one.
function CollapsiblePanel({
  children,
  defaultOpen,
  icon: Icon,
  title,
}: {
  children: React.ReactNode;
  defaultOpen: boolean;
  icon: LucideIcon;
  title: string;
}) {
  // Only the first render decides; filling the panel in never snaps it shut.
  const [initiallyOpen] = useState(defaultOpen);
  return (
    <Card className="gap-3 rounded-xl bg-surface-raised p-3 shadow-raised">
      <details className="group/panel" open={initiallyOpen}>
        <summary className="flex cursor-pointer list-none items-center gap-2 text-section font-semibold text-foreground [&::-webkit-details-marker]:hidden">
          <Icon className="size-4 text-muted-foreground" aria-hidden="true" />
          {title}
          <ChevronDown
            aria-hidden="true"
            className="ms-auto size-4 text-muted-foreground transition-transform group-open/panel:rotate-180"
          />
        </summary>
        <div className="pt-3">{children}</div>
      </details>
    </Card>
  );
}
