import { ConnectionOptions } from "~/features/connection/connection-options";
import { queries } from "@voya/client/queries";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { voyaCommands } from "@voya/client/transport";
import { queryKeys } from "@voya/client/query-keys";
import { appErrorOfKind } from "@voya/client/errors";
import { validationFieldErrors } from "@voya/client/messages";
import { useRuntimeEventStore } from "@voya/client/runtime-event-store";
import type { DnsSettings } from "@voya/contracts";
import { cacheSavedDns } from "@voya/features/dns/dns-cache";
import { saveQueue } from "@voya/features/forms/save-queue";
import { runningConnectionKey } from "@voya/features/home/use-connection-ip";
import { useSettingsApplyStatus } from "@voya/features/settings/use-settings-apply-status";
import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "heroui-native/button";
import { FieldError } from "heroui-native/field-error";
import { Input } from "heroui-native/input";
import { Label } from "heroui-native/label";
import { ListGroup } from "heroui-native/list-group";
import { TextField } from "heroui-native/text-field";
import { Typography } from "heroui-native/text";
import { useState } from "react";
import { Keyboard } from "react-native";
import { DetailScreen } from "~/components/detail-screen";
import { Disclosure } from "~/components/disclosure";
import { ErrorNotice } from "~/components/error-notice";
import { PrimaryButton } from "~/components/primary-button";
import { useBusyAction } from "@voya/features/forms/use-busy-action";
import { SwitchRow } from "~/components/switch-row";
import { useUnsavedChanges } from "~/components/use-unsaved-changes";
import { SaveStatus } from "~/components/save-status";

// The hints are the desktop DNS pane's own words, so both shells explain the
// same setting the same way.
const FIELDS = [
  { key: "remote", labelKey: "panes.dns.remoteDns", hintKey: null, placeholderKey: "panes.dns.remoteDnsPlaceholder" },
  { key: "direct", labelKey: "panes.dns.directDns", hintKey: null, placeholderKey: "panes.dns.directDnsPlaceholder" },
  {
    key: "bootstrap",
    labelKey: "panes.dns.bootstrapDns",
    hintKey: "panes.dns.bootstrapHint",
    placeholderKey: "panes.dns.bootstrapDnsPlaceholder",
  },
] as const;
const SWITCHES = [
  { key: "fakeIp", labelKey: "panes.dns.fakeIp", hintKey: "panes.dns.fakeIpHint" },
  { key: "blockBindingQuery", labelKey: "panes.dns.blockBindingQuery", hintKey: "panes.dns.blockBindingQueryHint" },
] as const;

export function DnsScreen() {
  const { t } = useI18n();
  const client = useQueryClient();
  const query = useQuery(queries.dns);
  const [patch, setPatch] = useState<Partial<DnsSettings>>({});
  // One guard for saving and for applying: neither starts while the other runs.
  const { busy: saving, run } = useBusyAction();
  // What the running connection still owes the saved settings goes stale when
  // a save lands or the connection changes, so it is read again then — and not
  // while a save is running, when it would describe the settings before it.
  const connection = useRuntimeEventStore(runningConnectionKey);
  const apply = useSettingsApplyStatus({ refreshKey: saving ? null : (connection ?? "") });
  const [saved, setSaved] = useState(false);
  const [advanced, setAdvanced] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const [applyError, setApplyError] = useState<unknown>(null);
  const [fields, setFields] = useState<Record<string, string>>({});
  const form = query.data ? { ...query.data, ...patch } : null;
  const dirty = Object.keys(patch).length > 0;
  function edit(next: Partial<DnsSettings>) {
    setPatch((previous) => ({ ...previous, ...next }));
    setSaved(false);
    setError(null);
    setFields((previous) => Object.fromEntries(Object.entries(previous).filter(([key]) => !(key in next))));
  }
  const save = async () => (await run(saveEdits)) ?? false;
  async function saveEdits() {
    if (!form) return false;
    Keyboard.dismiss();
    setError(null);
    setFields({});
    try {
      // The app-settings bundle carries a copy of these fields. A save of it
      // still on its way — General saves as it is edited — read the old DNS
      // and would write it back over this one, so this waits its turn.
      await saveQueue(client).settled();
      const latest = await voyaCommands().loadDnsSettings();
      const result = await voyaCommands().saveDnsSettings({ ...latest, ...patch });
      await cacheSavedDns(client, result);
      setPatch({});
      setSaved(true);
      return true;
    } catch (failure) {
      setError(failure);
      const validation = appErrorOfKind(failure, "validation");
      if (validation) setFields(validationFieldErrors(t, validation.kind.issues));
      return false;
    }
  }
  useUnsavedChanges(dirty, saving, save);
  async function defaults() {
    try {
      const baseline = await voyaCommands().getDefaultDnsSettings();
      setError(null);
      setFields({});
      edit({
        remote: baseline.remote,
        direct: baseline.direct,
        bootstrap: baseline.bootstrap,
        fakeIp: baseline.fakeIp,
        blockBindingQuery: baseline.blockBindingQuery,
      });
    } catch (failure) {
      setError(failure);
    }
  }
  return (
    <DetailScreen>
      <ConnectionOptions />
      <Typography className="text-base text-subtle">{t("daily.dnsHint")}</Typography>
      <ErrorNotice error={query.error} retry={() => void query.refetch()} />
      {form ? (
        <>
          {/* Unsaved edits keep the fields open: collapsing them would hide what is about to be saved. */}
          <Disclosure title={t("mobile.advanced")} isExpanded={advanced || dirty} onExpandedChange={setAdvanced}>
            {FIELDS.map(({ key, labelKey, hintKey, placeholderKey }) => (
              <TextField key={key} isInvalid={Boolean(fields[key])}>
                <Label>{t(labelKey)}</Label>
                <Input
                  value={form[key] ?? ""}
                  placeholder={t(placeholderKey)}
                  onChangeText={(value) => edit({ [key]: value })}
                  editable={!saving}
                  autoCapitalize="none"
                  autoCorrect={false}
                  returnKeyType="done"
                  onSubmitEditing={Keyboard.dismiss}
                  accessibilityLabel={t(labelKey)}
                />
                {hintKey ? <Typography className="text-sm text-subtle">{t(hintKey)}</Typography> : null}
                <FieldError>{fields[key]}</FieldError>
              </TextField>
            ))}
            <ListGroup>
              {SWITCHES.map(({ key, labelKey, hintKey }, index) => (
                <SwitchRow
                  key={key}
                  last={index === SWITCHES.length - 1}
                  label={t(labelKey)}
                  description={hintKey ? t(hintKey) : undefined}
                  isDisabled={saving}
                  value={form[key] ?? false}
                  onChange={(value) => edit({ [key]: value })}
                />
              ))}
            </ListGroup>
          </Disclosure>
          {/* "Saved" only after a real save in this session: a page that opens
          already saying "Saved" teaches the user to ignore the line. */}
          <SaveStatus dirty={dirty} saved={saved} saving={saving} />
          <ErrorNotice error={error} message={t("mobile.saveFailed")} />
          {advanced || dirty ? (
            <>
              <PrimaryButton label={t("actions.save")} isDisabled={!dirty || saving} onPress={() => void save()} />
              <Button variant="secondary" isDisabled={saving} onPress={() => void defaults()}>
                <Button.Label>{t("mobile.restoreDns")}</Button.Label>
              </Button>
            </>
          ) : null}
        </>
      ) : null}
      <ErrorNotice error={apply.error} retry={() => void apply.refetch()} />
      <ErrorNotice error={applyError} message={t("notices.settingsSavedRuntimeUpdateFailed")} />
      {!dirty && apply.data?.connected && apply.data.action === "none" ? (
        <Typography accessibilityLiveRegion="polite" className="text-sm text-subtle">
          {t("mobile.applied")}
        </Typography>
      ) : null}
      {(saved || !dirty) && apply.data?.action !== undefined && apply.data.action !== "none" ? (
        <>
          <Typography className="text-base text-subtle">{t("mobile.applyPending")}</Typography>
          <Button
            variant="secondary"
            isDisabled={saving}
            onPress={() => {
              void run(async () => {
                setApplyError(null);
                await voyaCommands().applyPendingSettings();
                await client.invalidateQueries({ queryKey: queryKeys.appSettings });
              }).catch(setApplyError);
            }}
          >
            <Button.Label>{t("mobile.apply")}</Button.Label>
          </Button>
        </>
      ) : null}
    </DetailScreen>
  );
}
