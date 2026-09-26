import { useState } from "react";
import type * as React from "react";
import { Server, Tag, TriangleAlert } from "lucide-react";

import { Alert, AlertDescription } from "@voya/ui/components/alert";
import { Button } from "@voya/ui/components/button";
import {
  Dialog,
  DialogDescription,
  DialogBody,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  ScrollableDialogContent,
} from "@voya/ui/components/dialog";
import { SelectField } from "@voya/ui/components/form-fields";
import { useI18n } from "@voya/i18n/use-i18n";
import type { Profile } from "@voya/contracts";
import {
  translateFieldErrors,
  zodIssuesToErrorMap,
  type FieldErrorMap,
} from "@voya/features/forms/zod-errors";

import {
  isProfileKind,
  PROFILE_PROTOCOL_OPTIONS,
} from "@voya/features/profiles/profile-constants";
import {
  createDefaultDraft,
  draftFromProfile,
  parseProfileDraft,
  profileFromDraft,
  type ProfileDraft,
} from "@voya/features/profiles/profile-draft";
import { DraftTextField, Panel, type ProfilePanelProps } from "./profile-form-fields";
import { ProtocolPanel } from "./profile-protocol-panel";
import { SecurityPanel } from "./profile-security-panel";
import { TransportPanel } from "./profile-transport-panel";

type ProfileDialogProps = {
  mode: "create" | "edit";
  onCloseFocus?: () => void;
  onOpenChange: (open: boolean) => void;
  onSubmit: (profile: Profile) => Promise<void>;
  open: boolean;
  profile?: Profile | null;
  // Backend rejection of the last save. The dialog stays open on failure so the
  // in-progress edits survive, and the message is shown here instead of behind
  // the modal.
  saveError?: string | null;
};

export function ProfileDialog({
  onCloseFocus,
  mode,
  onOpenChange,
  onSubmit,
  open,
  profile,
  saveError,
}: ProfileDialogProps) {
  const formKey = `${mode}:${profile?.id ?? "new"}:${open ? "open" : "closed"}`;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <ProfileDialogForm
        key={formKey}
        onCloseFocus={onCloseFocus}
        mode={mode}
        onOpenChange={onOpenChange}
        onSubmit={onSubmit}
        profile={profile}
        saveError={saveError}
      />
    </Dialog>
  );
}

function ProfileDialogForm({
  onCloseFocus,
  mode,
  onOpenChange,
  onSubmit,
  profile,
  saveError,
}: Omit<ProfileDialogProps, "open">) {
  const { t } = useI18n();
  const [draft, setDraft] = useState<ProfileDraft>(() =>
    profile ? draftFromProfile(profile) : createDefaultDraft("vmess"),
  );
  const [fieldErrors, setFieldErrors] = useState<FieldErrorMap>({});
  const [pending, setPending] = useState(false);

  function update<Key extends keyof ProfileDraft>(
    key: Key,
    value: ProfileDraft[Key],
  ) {
    setDraft((current) => ({ ...current, [key]: value }));
  }

  const panel: ProfilePanelProps = {
    draft,
    errors: translateFieldErrors(t, fieldErrors),
    onChange: update,
  };

  async function submitForm(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending) return;
    const parsed = parseProfileDraft(draft);
    if (!parsed.success) {
      setFieldErrors(zodIssuesToErrorMap(parsed.error));
      // Hidden fields keep their session draft. Reveal their section on errors.
      document
        .querySelectorAll<HTMLDetailsElement>("#profile-form details")
        .forEach((detail) => {
          detail.open = true;
        });
      requestAnimationFrame(() =>
        document
          .querySelector<HTMLElement>('#profile-form [aria-invalid="true"]')
          ?.focus(),
      );
      return;
    }
    setFieldErrors({});
    setPending(true);
    try {
      await onSubmit(profileFromDraft(parsed.data));
    } finally {
      setPending(false);
    }
  }

  return (
    <ScrollableDialogContent
      closeLabel={t("actions.close")}
      onCloseAutoFocus={
        onCloseFocus
          ? (event) => {
              event.preventDefault();
              onCloseFocus();
            }
          : undefined
      }
      width="68rem"
    >
      <DialogHeader>
        <DialogTitle className="flex items-center gap-2">
          <Server className="size-4" aria-hidden="true" />
          {mode === "edit"
            ? t("panes.profiles.dialog.editTitle")
            : t("panes.profiles.dialog.addTitle")}
        </DialogTitle>
        <DialogDescription className="sr-only">
          {t("panes.profiles.dialog.description")}
        </DialogDescription>
      </DialogHeader>
      <DialogBody>
        <form
          className="min-h-0"
          id="profile-form"
          onSubmit={(event) => void submitForm(event)}
        >
          <div className="grid gap-4">
            <Panel icon={Tag} title={t("panes.profiles.panels.profile")}>
              <div className="grid gap-3 lg:grid-cols-[14rem_1fr]">
                <SelectField
                  label={t("panes.profiles.fields.protocol")}
                  onChange={(value) => {
                    if (isProfileKind(value)) update("kind", value);
                  }}
                  options={PROFILE_PROTOCOL_OPTIONS}
                  value={draft.kind}
                />

                <DraftTextField
                  {...panel}
                  label={t("panes.profiles.fields.remarks")}
                  name="remarks"
                />
              </div>

              <div className="grid gap-3 lg:grid-cols-[1fr_7rem]">
                <DraftTextField
                  {...panel}
                  label={t("panes.profiles.fields.address")}
                  name="address"
                />
                <DraftTextField
                  {...panel}
                  inputMode="numeric"
                  label={t("panes.profiles.fields.port")}
                  name="port"
                />
              </div>
            </Panel>

            <ProtocolPanel {...panel} />
            {/* WireGuard carries its own UDP framing: no transport settings. */}
            {draft.kind !== "wireGuard" ? <TransportPanel {...panel} /> : null}
            <SecurityPanel {...panel} />
          </div>
        </form>

        {saveError ? (
          <Alert className="mx-6 mb-2 w-auto" variant="destructive">
            <TriangleAlert aria-hidden="true" />
            <AlertDescription>{saveError}</AlertDescription>
          </Alert>
        ) : null}
      </DialogBody>
      <DialogFooter>
        <Button
          disabled={pending}
          onClick={() => onOpenChange(false)}
          type="button"
          variant="outline"
        >
          {t("panes.profiles.dialog.cancel")}
        </Button>
        <Button disabled={pending} form="profile-form" type="submit">
          {t("panes.profiles.dialog.save")}
        </Button>
      </DialogFooter>
    </ScrollableDialogContent>
  );
}
