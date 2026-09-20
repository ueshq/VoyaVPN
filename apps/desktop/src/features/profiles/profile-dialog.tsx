import { zodResolver } from "@hookform/resolvers/zod";
import { Server, Tag, TriangleAlert } from "lucide-react";
import { useForm, useWatch } from "react-hook-form";

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
import { useI18n } from "@voya/i18n/use-i18n";
import type { Profile, ProfileKind } from "@/ipc/bindings";

import { localizeProfileProtocols } from "@voya/features/profiles/profile-constants";
import {
  Panel,
  ProfileFields,
  SelectField,
  TextField,
} from "./profile-form-fields";
import { profileValidationMessage } from "@voya/features/profiles/profile-form-utils";
import { profileFormSchema, activeProfileFormValues, type ParsedProfileFormValues, type ProfileFormValues } from "@voya/features/profiles/profile-form-schema";
import { createDefaultProfile, normalizeProfileForForm, prepareProfileForSave } from "@voya/features/profiles/profile-form-values";
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
  const form = useForm<ProfileFormValues, unknown, ParsedProfileFormValues>({
    defaultValues: profile
      ? normalizeProfileForForm(profile)
      : createDefaultProfile(),
    mode: "onBlur",
    resolver: (values, context, options) =>
      zodResolver(profileFormSchema)(
        activeProfileFormValues(values),
        context,
        options,
      ),
  });
  const {
    formState: { errors, isSubmitting },
    handleSubmit,
    register,
  } = form;
  const configType = useWatch({
    control: form.control,
    name: "configType",
  }) as ProfileKind;
  const security =
    useWatch({ control: form.control, name: "streamSecurity" }) ?? "";

  const submit = handleSubmit(
    async (values) => {
      await onSubmit(prepareProfileForSave(values));
    },
    () => {
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
    },
  );

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
        <ProfileFields errors={errors}>
          <form
            className="min-h-0"
            id="profile-form"
            onSubmit={(event) => void submit(event)}
          >
            <div className="grid gap-4">
              <Panel icon={Tag} title={t("panes.profiles.panels.profile")}>
                <div className="grid gap-3 lg:grid-cols-[14rem_1fr]">
                  <SelectField
                    control={form.control}
                    label={t("panes.profiles.fields.protocol")}
                    name="configType"
                    options={localizeProfileProtocols(t)}
                  />

                  <TextField
                    error={profileValidationMessage(errors.remarks?.message, t)}
                    label={t("panes.profiles.fields.remarks")}
                    {...register("remarks")}
                  />
                </div>

                <div className="grid gap-3 lg:grid-cols-[1fr_7rem]">
                  <TextField
                    error={profileValidationMessage(errors.address?.message, t)}
                    label={t("panes.profiles.fields.address")}
                    {...register("address")}
                  />
                  <TextField
                    error={profileValidationMessage(errors.port?.message, t)}
                    inputMode="numeric"
                    label={t("panes.profiles.fields.port")}
                    type="number"
                    {...register("port", { valueAsNumber: true })}
                  />
                </div>
              </Panel>

              <ProtocolPanel
                configType={configType}
                control={form.control}
                passwordError={profileValidationMessage(
                  errors.password?.message,
                  t,
                )}
                register={register}
                usernameError={profileValidationMessage(
                  errors.username?.message,
                  t,
                )}
              />
              {configType !== "wireGuard" ? (
                <TransportPanel control={form.control} register={register} />
              ) : null}
              <SecurityPanel
                control={form.control}
                register={register}
                security={security}
              />
            </div>
          </form>
        </ProfileFields>

        {saveError ? (
          <Alert className="mx-6 mb-2 w-auto" variant="destructive">
            <TriangleAlert aria-hidden="true" />
            <AlertDescription>{saveError}</AlertDescription>
          </Alert>
        ) : null}
      </DialogBody>
      <DialogFooter>
        <Button
          disabled={isSubmitting}
          onClick={() => onOpenChange(false)}
          type="button"
          variant="outline"
        >
          {t("panes.profiles.dialog.cancel")}
        </Button>
        <Button disabled={isSubmitting} form="profile-form" type="submit">
          {t("panes.profiles.dialog.save")}
        </Button>
      </DialogFooter>
    </ScrollableDialogContent>
  );
}
