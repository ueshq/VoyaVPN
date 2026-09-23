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
import { SelectField, TextField } from "@voya/ui/components/form-fields";
import { useI18n } from "@voya/i18n/use-i18n";
import type { Profile, ProfileKind } from "@voya/contracts";
import type { TranslationFunction } from "@voya/i18n/core";
import type { ZodError } from "@voya/features/forms/zod-errors";

import { localizeProfileProtocols } from "@voya/features/profiles/profile-constants";
import {
  Panel,
} from "./profile-form-fields";
import {
  toEditorForm,
  toSchemaValues,
  type ProfileEditorForm,
  type ProfileFieldErrors,
} from "./profile-editor-form";
import { profileValidationMessage } from "@voya/features/profiles/profile-form-utils";
import {
  activeProfileFormValues,
  profileFormSchema,
} from "@voya/features/profiles/profile-form-schema";
import {
  createDefaultProfile,
  normalizeProfileForForm,
  prepareProfileForSave,
} from "@voya/features/profiles/profile-form-values";
import { ProtocolPanel } from "./profile-protocol-panel";
import { SecurityPanel } from "./profile-security-panel";
import { TransportPanel } from "./profile-transport-panel";

type ProfileDialogProps = {
  mode: "create" | "edit";
  onCloseFocus?: () => void;
  onOpenChange: (open: boolean) => void;
  onSubmit: (
    profile: ReturnType<typeof prepareProfileForSave>,
  ) => Promise<void>;
  open: boolean;
  profile?: Profile | null;
  // Backend rejection of the last save. The dialog stays open on failure so the
  // in-progress edits survive, and the message is shown here instead of behind
  // the modal.
  saveError?: string | null;
};

/** Zod issue paths, dotted, → the locale string the editor renders. */
function issueMessages(error: ZodError, t: TranslationFunction): ProfileFieldErrors {
  const messages: ProfileFieldErrors = {};
  for (const issue of error.issues) {
    messages[issue.path.join(".")] = profileValidationMessage(issue.message, t);
  }
  return messages;
}

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
  const [form, setForm] = useState<ProfileEditorForm>(() =>
    toEditorForm(
      profile ? normalizeProfileForForm(profile) : createDefaultProfile(),
    ),
  );
  const [fieldErrors, setFieldErrors] = useState<ProfileFieldErrors>({});
  const [pending, setPending] = useState(false);
  const configType = form.configType as ProfileKind;
  const security = form.streamSecurity;

  function update<Key extends keyof ProfileEditorForm>(
    key: Key,
    value: ProfileEditorForm[Key],
  ) {
    setForm((current) => ({ ...current, [key]: value }));
  }

  function updateOption<Key extends keyof ProfileEditorForm["protocolOptions"]>(
    key: Key,
    value: ProfileEditorForm["protocolOptions"][Key],
  ) {
    setForm((current) => ({
      ...current,
      protocolOptions: { ...current.protocolOptions, [key]: value },
    }));
  }

  function updateTransport<
    Key extends keyof ProfileEditorForm["transportOptions"],
  >(key: Key, value: ProfileEditorForm["transportOptions"][Key]) {
    setForm((current) => ({
      ...current,
      transportOptions: { ...current.transportOptions, [key]: value },
    }));
  }

  async function submitForm(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (pending) return;
    const parsed = profileFormSchema.safeParse(
      activeProfileFormValues(toSchemaValues(form)),
    );
    if (!parsed.success) {
      setFieldErrors(issueMessages(parsed.error, t));
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
      await onSubmit(prepareProfileForSave(parsed.data));
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
                  onChange={(value) => update("configType", value)}
                  options={localizeProfileProtocols(t).map(
                    ({ label, value }) => ({ label, value }),
                  )}
                  value={form.configType}
                />

                <TextField
                  error={fieldErrors.remarks}
                  label={t("panes.profiles.fields.remarks")}
                  onChange={(value) => update("remarks", value)}
                  value={form.remarks}
                />
              </div>

              <div className="grid gap-3 lg:grid-cols-[1fr_7rem]">
                <TextField
                  error={fieldErrors.address}
                  label={t("panes.profiles.fields.address")}
                  onChange={(value) => update("address", value)}
                  value={form.address}
                />
                <TextField
                  error={fieldErrors.port}
                  inputMode="numeric"
                  label={t("panes.profiles.fields.port")}
                  onChange={(value) => update("port", value)}
                  value={form.port}
                />
              </div>
            </Panel>

            <ProtocolPanel
              configType={configType}
              errors={fieldErrors}
              form={form}
              onFieldChange={(key, value) => update(key, value)}
              onOptionChange={updateOption}
            />
            {configType !== "wireGuard" ? (
              <TransportPanel
                errors={fieldErrors}
                form={form}
                onFieldChange={(key, value) => update(key, value)}
                onTransportChange={updateTransport}
              />
            ) : null}
            <SecurityPanel
              errors={fieldErrors}
              form={form}
              onFieldChange={(key, value) => update(key, value)}
              security={security}
            />
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
