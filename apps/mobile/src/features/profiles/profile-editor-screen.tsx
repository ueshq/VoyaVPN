import { useQuery, useQueryClient } from "@tanstack/react-query";
import type { NativeStackScreenProps } from "@react-navigation/native-stack";
import { refreshQueries } from "@voya/client/queries";
import { profileDetailsQueryKey, queryKeys } from "@voya/client/query-keys";
import { voyaCommands } from "@voya/client/transport";
import { getProtocolLabel } from "@voya/features/profiles/profile-constants";
import {
  draftFromProfile,
  parseProfileDraft,
  profileFromDraft,
} from "@voya/features/profiles/profile-draft";
import {
  translateFieldErrors,
  zodIssuesToErrorMap,
} from "@voya/features/forms/zod-errors";
import type { Profile } from "@voya/contracts";
import { useI18n } from "@voya/i18n/use-i18n";
import { TRANSPORT_OPTIONS } from "@voya/features/profiles/profile-constants";
import { FieldError } from "heroui-native/field-error";
import { Input } from "heroui-native/input";
import { Label } from "heroui-native/label";
import { ListGroup } from "heroui-native/list-group";
import { TextField } from "heroui-native/text-field";
import { Typography } from "heroui-native/text";
import { useState } from "react";
import { View } from "react-native";
import type { RootRoutes } from "~/app/navigation";

import { DetailScreen } from "~/components/detail-screen";
import { ErrorNotice } from "~/components/error-notice";
import { ListRow } from "~/components/list-row";
import { useBusyAction } from "~/components/use-busy-action";
import { useUnsavedChanges } from "~/components/use-unsaved-changes";
import { SaveStatus } from "~/components/save-status";
import { PrimaryButton } from "~/components/primary-button";

/**
 * Edits a node's common fields.
 *
 * Everything protocol-, transport- and TLS-specific stays as the node already
 * has it: the shared `ProfileDraft` round-trips the untouched parts verbatim,
 * so saving rewrites only the name, address and port. The rest is shown as a
 * read-only summary; a full editor is the desktop's profile dialog.
 */
export function ProfileEditorScreen({ route }: NativeStackScreenProps<RootRoutes, "editProfile">) {
  const { t } = useI18n();
  const details = useQuery({
    queryFn: () => voyaCommands().getProfile(route.params.id),
    // Under the profiles root, so a save — here or anywhere — refreshes it.
    queryKey: profileDetailsQueryKey(route.params.id),
  });

  return (
    // The navigation bar already names this page; every other stack page
    // relies on that title alone, so an in-page one just said it twice.
    <DetailScreen accessibilityLabel={t("mobile.editNode")}>
      {details.error ? (
        <ErrorNotice
          error={details.error}
          retry={() => void details.refetch()}
        />
      ) : null}
      {details.data ? (
        <Editor key={details.data.profile.id} profile={details.data.profile} />
      ) : details.isPending ? (
        <Typography className="text-base text-subtle">
          {t("panes.profiles.loadingNodes")}
        </Typography>
      ) : null}
    </DetailScreen>
  );
}

function Editor({ profile }: { profile: Profile }) {
  const { t } = useI18n();
  const client = useQueryClient();
  const [name, setName] = useState(profile.remarks);
  const [address, setAddress] = useState(profile.protocol.server.address);
  const [port, setPort] = useState(String(profile.protocol.server.port));
  const [baseline, setBaseline] = useState({ address, name, port });
  const { busy, run } = useBusyAction();
  const [error, setError] = useState<unknown>(null);
  const [fields, setFields] = useState({
    address: false,
    name: false,
    port: false,
  });
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
  const dirty =
    name !== baseline.name ||
    address !== baseline.address ||
    port !== baseline.port;

  async function save(): Promise<boolean> {
    return (await run(saveDraft)) ?? false;
  }

  async function saveDraft(): Promise<boolean> {
    setError(null);
    setFields({ address: false, name: false, port: false });
    const draft = {
      ...draftFromProfile(profile),
      address: address.trim(),
      port: port.trim(),
      remarks: name.trim(),
    };
    const parsed = parseProfileDraft(draft);
    if (!parsed.success) {
      const translated = translateFieldErrors(
        t,
        zodIssuesToErrorMap(parsed.error),
      );
      setFieldErrors(translated);
      setFields({
        address: Boolean(translated.address),
        name: Boolean(translated.remarks),
        port: Boolean(translated.port),
      });
      return false;
    }
    try {
      await voyaCommands().saveProfile(profileFromDraft(parsed.data));
      setBaseline({ address, name, port });
      // The node list and this node's own details: what the save changed.
      await refreshQueries(client, queryKeys.profiles);
      return true;
    } catch (failure) {
      setError(failure);
      return false;
    }
  }

  useUnsavedChanges(dirty, busy, save);

  const transportLabel = profile?.transport
    ? (TRANSPORT_OPTIONS.find(
        (option) => option.value === profile.transport?.kind,
      )?.label ?? profile.transport.kind)
    : t("mobile.editorTransportNone");

  return (
    <>
      <Typography className="text-base text-subtle">
        {t("mobile.editorFieldsHint")}
      </Typography>
      <TextField isInvalid={fields.name}>
        <Label>{t("mobile.name")}</Label>
        <Input
          accessibilityLabel={t("mobile.name")}
          value={name}
          onChangeText={setName}
          editable={!busy}
        />
        <FieldError>{fieldErrors.remarks}</FieldError>
      </TextField>
      <TextField isInvalid={fields.address}>
        <Label>{t("mobile.address")}</Label>
        <Input
          accessibilityLabel={t("mobile.address")}
          value={address}
          onChangeText={setAddress}
          editable={!busy}
          autoCorrect={false}
          autoCapitalize="none"
        />
        <FieldError>{fieldErrors.address}</FieldError>
      </TextField>
      <TextField isInvalid={fields.port}>
        <Label>{t("mobile.port")}</Label>
        <Input
          accessibilityLabel={t("mobile.port")}
          value={port}
          onChangeText={setPort}
          editable={!busy}
          keyboardType="number-pad"
        />
        <FieldError>{fieldErrors.port}</FieldError>
      </TextField>
      <SaveStatus dirty={dirty} saving={busy} />
      <PrimaryButton label={t("actions.save")} isDisabled={!dirty || busy} onPress={() => void save()} />
      <ErrorNotice
        error={error}
        message={dirty ? t("mobile.saveFailed") : undefined}
      />

      {/* What the editor deliberately does not touch, stated so it does not
          read as an omission. */}
      <View className="gap-2">
        <Typography className="px-1 text-sm font-medium text-subtle">
          {t("mobile.editorSummary")}
        </Typography>
        <ListGroup>
          <ListRow
            title={t("mobile.editorProtocol")}
            description={profile ? getProtocolLabel(profile.protocol.kind) : ""}
          />
          <ListRow
            title={t("mobile.editorTransport")}
            description={transportLabel}
          />
          <ListRow
            last
            title={t("mobile.editorTls")}
            description={
              profile?.tls
                ? profile.tls.mode === "reality"
                  ? "REALITY"
                  : "TLS"
                : t("mobile.editorTransportNone")
            }
          />
        </ListGroup>
      </View>
    </>
  );
}
