import { useQuery, useQueryClient } from "@tanstack/react-query";
import { voyaCommands } from "@voya/client/transport";
import { queryKeys } from "@voya/client/query-keys";
import { appErrorOfKind } from "@voya/client/errors";
import { validationFieldErrors } from "@voya/client/messages";
import type { DnsSettings } from "@voya/contracts";
import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "heroui-native/button";
import { Input } from "heroui-native/input";
import { Switch } from "heroui-native/switch";
import { Typography } from "heroui-native/text";
import { useRef, useState } from "react";
import { Keyboard, View } from "react-native";
import { DetailScreen } from "~/components/detail-screen";
import { ErrorNotice } from "~/components/error-notice";
import { ListCard } from "~/components/list-card";
import { ListRow } from "~/components/list-row";
import { useUnsavedChanges } from "~/components/use-unsaved-changes";

const FIELDS = [{ key: "remote", labelKey: "panes.dns.remoteDns" }, { key: "direct", labelKey: "panes.dns.directDns" }, { key: "bootstrap", labelKey: "panes.dns.bootstrapDns" }] as const;
const SWITCHES = [{ key: "fakeIp", labelKey: "panes.dns.fakeIp" }, { key: "blockBindingQuery", labelKey: "panes.dns.blockBindingQuery" }] as const;

export function DnsScreen() {
  const { t } = useI18n();
  const client = useQueryClient();
  const query = useQuery({ queryKey: queryKeys.dns, queryFn: () => voyaCommands().loadDnsSettings() });
  const apply = useQuery({ queryKey: queryKeys.settingsApply, queryFn: () => voyaCommands().getSettingsApplyStatus() });
  const [patch, setPatch] = useState<Partial<DnsSettings>>({});
  const [saving, setSaving] = useState(false);
  const savingRef = useRef(false);
  const [saved, setSaved] = useState(false);
  const [advanced, setAdvanced] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const [applyError, setApplyError] = useState<unknown>(null);
  const [fields, setFields] = useState<Record<string, string>>({});
  const form = query.data ? { ...query.data, ...patch } : null;
  const dirty = Object.keys(patch).length > 0;
  function edit(next: Partial<DnsSettings>) { setPatch((previous) => ({ ...previous, ...next })); setSaved(false); setError(null); setFields((previous) => Object.fromEntries(Object.entries(previous).filter(([key]) => !(key in next)))); }
  async function save() {
    if (!form || savingRef.current) return false;
    Keyboard.dismiss();
    savingRef.current = true; setSaving(true); setError(null); setFields({});
    try {
      const latest = await voyaCommands().loadDnsSettings();
      const result = await voyaCommands().saveDnsSettings({ ...latest, ...patch });
      client.setQueryData(queryKeys.dns, result);
      await client.invalidateQueries({ queryKey: queryKeys.appSettings });
      setPatch({}); setSaved(true); return true;
    } catch (failure) {
      setError(failure);
      const validation = appErrorOfKind(failure, "validation");
      if (validation) setFields(validationFieldErrors(t, validation.kind.issues));
      return false;
    } finally { savingRef.current = false; setSaving(false); }
  }
  useUnsavedChanges(dirty, saving, save);
  async function defaults() {
    try {
      const baseline = await voyaCommands().getDefaultDnsSettings();
      setError(null); setFields({});
      edit({ remote: baseline.remote, direct: baseline.direct, bootstrap: baseline.bootstrap, fakeIp: baseline.fakeIp, blockBindingQuery: baseline.blockBindingQuery });
    } catch (failure) { setError(failure); }
  }
  return <DetailScreen>
    <ErrorNotice error={query.error} retry={() => void query.refetch()} />
    <Button variant="secondary" className="min-h-12 h-auto" isDisabled={saving} onPress={() => void defaults()}><Button.Label>{t("mobile.recommended")}</Button.Label></Button>
    <Button variant="ghost" className="min-h-12 h-auto" onPress={() => setAdvanced(!advanced)} accessibilityState={{ expanded: advanced }}><Button.Label>{t("mobile.advanced")}</Button.Label></Button>
    {form ? <>
      {advanced || dirty ? FIELDS.map(({ key, labelKey }) => <View key={key} className="gap-2">
        <Typography className="text-base text-foreground">{t(labelKey)}</Typography>
        <Input className="min-h-12 h-auto" value={form[key] ?? ""} onChangeText={(value) => edit({ [key]: value })} editable={!saving} autoCapitalize="none" autoCorrect={false} returnKeyType="done" onSubmitEditing={Keyboard.dismiss} accessibilityLabel={t(labelKey)} isInvalid={Boolean(fields[key])} />
        {fields[key] ? <Typography className="text-sm text-danger">{fields[key]}</Typography> : null}
      </View>) : <Typography className="text-base text-subtle">{FIELDS.map(({ key, labelKey }) => `${t(labelKey)}: ${form[key] ?? "—"}`).join("\n")}</Typography>}
      {advanced ? <ListCard>{SWITCHES.map(({ key, labelKey }, index) => <ListRow key={key} last={index === SWITCHES.length - 1} title={t(labelKey)} trailing={<Switch accessibilityLabel={t(labelKey)} isDisabled={saving} isSelected={form[key] ?? false} onSelectedChange={(value) => edit({ [key]: value })} />} />)}</ListCard> : null}
      <Typography accessibilityLiveRegion="polite" className="text-sm text-subtle">{saving ? t("mobile.saving") : dirty ? t("mobile.unsaved") : t("mobile.saved")}</Typography>
      <ErrorNotice error={error} message={t("mobile.saveFailed")} />
      <Button className="min-h-12 h-auto" isDisabled={!dirty || saving} onPress={() => void save()}><Button.Label>{t("actions.save")}</Button.Label></Button>
      <Button variant="ghost" className="min-h-12 h-auto" isDisabled={saving} onPress={() => void defaults()}><Button.Label>{t("mobile.restoreDns")}</Button.Label></Button>
    </> : null}
    <ErrorNotice error={apply.error} retry={() => void apply.refetch()} />
    <ErrorNotice error={applyError} message={t("notices.settingsSavedRuntimeUpdateFailed")} />
    {!dirty && apply.data?.connected && apply.data.action === "none" ? <Typography accessibilityLiveRegion="polite" className="text-sm text-subtle">{t("mobile.applied")}</Typography> : null}
    {(saved || !dirty) && apply.data?.action !== undefined && apply.data.action !== "none" ? <>
      <Typography className="text-base text-subtle">{t("mobile.applyPending")}</Typography>
      <Button variant="secondary" className="min-h-12 h-auto" isDisabled={saving} onPress={() => {
        if (savingRef.current) return;
        savingRef.current = true; setSaving(true); setApplyError(null);
        void voyaCommands().applyPendingSettings().then(() => client.invalidateQueries({ queryKey: queryKeys.appSettings })).catch(setApplyError).finally(() => { savingRef.current = false; setSaving(false); });
      }}><Button.Label>{t("mobile.apply")}</Button.Label></Button>
    </> : null}
  </DetailScreen>;
}
