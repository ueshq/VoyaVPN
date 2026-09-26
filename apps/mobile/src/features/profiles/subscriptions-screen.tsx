import { queries } from "@voya/client/queries";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import type { NativeStackScreenProps } from "@react-navigation/native-stack";
import { voyaCommands } from "@voya/client/transport";
import { queryKeys } from "@voya/client/query-keys";
import type { Subscription, SubscriptionMetadata } from "@voya/contracts";
import { useI18n } from "@voya/i18n/use-i18n";
import { formatBytes } from "@voya/utils/formatting";
import { formatSubscriptionUpdateSummary, subscriptionUpdateMessages } from "@voya/features/subscriptions/subscription-update-result";
import { Button } from "heroui-native/button";
import { FieldError } from "heroui-native/field-error";
import { Input } from "heroui-native/input";
import { Label } from "heroui-native/label";
import { ListGroup } from "heroui-native/list-group";
import { TextField } from "heroui-native/text-field";
import { Typography } from "heroui-native/text";
import { useRef, useState } from "react";
import { Alert } from "react-native";
import { openPage, type RootRoutes } from "~/app/navigation";
import { DetailScreen } from "~/components/detail-screen";
import { Banner } from "~/components/banner";
import { ErrorNotice } from "~/components/error-notice";
import { ListRow } from "~/components/list-row";
import { useUnsavedChanges } from "~/components/use-unsaved-changes";
import { deleteSafely } from "./delete-safely";

function useSources() {
  const sources = useQuery(queries.subscriptions);
  const metadata = useQuery(queries.subscriptionMetadata);
  return { sources, metadata };
}

export function SubscriptionsScreen() {
  const { t } = useI18n();
  const { sources } = useSources();
  const client = useQueryClient();
  const [busy, setBusy] = useState(false);
  const updatePending = useRef(false);
  const [failedIds, setFailedIds] = useState<string[]>([]);
  const [error, setError] = useState<unknown>(null);
  const [message, setMessage] = useState<string | null>(null);
  async function update(ids: string[] | null) {
    if (updatePending.current) return;
    updatePending.current = true;
    setBusy(true); setError(null); setMessage(null);
    const failed = new Set<string>();
    try {
      const targets = ids ?? [null];
      const summaries: string[] = [];
      const failures: string[] = [];
      for (const id of targets) {
        try {
          const result = await voyaCommands().updateSubscriptions(id, true, null);
          result.outcomes.filter((item) => item.status === "failed").forEach((item) => failed.add(item.subscriptionId));
          if (result.updated > 0) summaries.push(formatSubscriptionUpdateSummary(result, t));
          const reason = subscriptionUpdateMessages(result, t);
          if (reason) failures.push(reason);
        } catch (failure) {
          (id ? [id] : sources.data?.map((item) => item.id) ?? []).forEach((value) => failed.add(value));
          setError(failure);
        }
      }
      setMessage([...summaries, ...failures].join("\n"));
      await client.invalidateQueries();
    } finally { setFailedIds([...failed]); updatePending.current = false; setBusy(false); }
  }
  return <DetailScreen>
    <Button onPress={() => openPage("import")}><Button.Label>{t("mobile.add")}</Button.Label></Button>
    <Button variant="secondary" isDisabled={busy || !sources.data?.length} onPress={() => void update(null)}><Button.Label>{t("panes.profiles.toolbar.updateAllSubscriptions")}</Button.Label></Button>
    {failedIds.length ? <Button variant="secondary" isDisabled={busy} onPress={() => void update(failedIds)}><Button.Label>{t("mobile.retryFailed")}</Button.Label></Button> : null}
    {message ? <Banner status={failedIds.length ? "warning" : "info"} liveRegion message={message} /> : null}
    <ErrorNotice error={error ?? sources.error} retry={() => void sources.refetch()} />
    <ListGroup>{sources.data?.map((item, index, all) => <ListRow key={item.id} title={item.remarks}
      description={failedIds.includes(item.id) ? t("mobile.updateFailed") : undefined} descriptionLines={0}
      last={index === all.length - 1} chevron onPress={() => openPage("subscription", { id: item.id })} />)}</ListGroup>
  </DetailScreen>;
}

export function SubscriptionScreen({ route, navigation }: NativeStackScreenProps<RootRoutes, "subscription">) {
  const { t } = useI18n();
  const { sources, metadata } = useSources();
  const item = sources.data?.find((source) => source.id === route.params.id);
  return item ? <SubscriptionEditor key={item.id} item={item} metadata={metadata.data?.find((meta) => meta.subscriptionId === item.id)} close={() => navigation.goBack()} />
    : <DetailScreen><ErrorNotice error={sources.error} retry={() => void sources.refetch()} />{!sources.error ? <Typography className="text-base text-subtle">{sources.isPending ? t("options.loading") : t("mobile.sourceMissing")}</Typography> : null}</DetailScreen>;
}

function SubscriptionEditor({ item, metadata, close }: { item: Subscription; metadata?: SubscriptionMetadata; close: () => void }) {
  const { t, language } = useI18n();
  const client = useQueryClient();
  const [name, setName] = useState(item.remarks);
  const [url, setUrl] = useState(item.url);
  const [baseline, setBaseline] = useState({ name, url });
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const [error, setError] = useState<unknown>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [messageFailed, setMessageFailed] = useState(false);
  const [fields, setFields] = useState({ name: false, url: false });
  const dirty = name !== baseline.name || url !== baseline.url;
  const nodes = useQuery(queries.profileList);
  const members = nodes.data?.entries.filter((entry) => entry.profile.subscriptionId === item.id) ?? [];
  async function save() {
    if (busyRef.current) return false;
    let validUrl = false;
    try { const parsed = new URL(url); validUrl = ["https:", "http:"].includes(parsed.protocol) && Boolean(parsed.hostname); } catch { /* field error below */ }
    setFields({ name: !name.trim(), url: !validUrl });
    if (!name.trim() || !validUrl) { setError(t("mobile.saveFailed")); return false; }
    busyRef.current = true; setBusy(true); setError(null);
    try {
      const current = (await voyaCommands().listSubscriptions()).find((source) => source.id === item.id);
      if (!current) throw new Error("Subscription no longer exists");
      await voyaCommands().saveSubscription({ ...current, remarks: name.trim(), url: url.trim() });
      setBaseline({ name, url }); setMessageFailed(false); setMessage(t("mobile.saved"));
      await client.invalidateQueries({ queryKey: queryKeys.subscriptions });
      return true;
    } catch (failure) { setError(failure); return false; }
    finally { busyRef.current = false; setBusy(false); }
  }
  useUnsavedChanges(dirty, busy, save);
  async function refresh() {
    if (busyRef.current || dirty) return;
    busyRef.current = true; setBusy(true); setError(null);
    try {
      const result = await voyaCommands().updateSubscriptions(item.id, true, null);
      const failure = subscriptionUpdateMessages(result, t);
      setMessageFailed(Boolean(failure));
      setMessage(failure || formatSubscriptionUpdateSummary(result, t));
      await client.invalidateQueries();
    } catch (failure) { setError(failure); }
    finally { busyRef.current = false; setBusy(false); }
  }
  function confirmDelete() {
    Alert.alert(t("mobile.deleteConfirm"), `${t("mobile.deleteCount", { count: members.length })}\n${t("mobile.deleteHint")}`, [
      { text: t("actions.cancel"), style: "cancel" },
      { text: t("actions.delete"), style: "destructive", onPress: () => {
        if (busyRef.current) return;
        busyRef.current = true; setBusy(true);
        void (async () => {
          const currentNodes = await voyaCommands().listProfileSummaries();
          await deleteSafely(currentNodes.entries.filter((entry) => entry.profile.subscriptionId === item.id).map((entry) => entry.profile.id), () => voyaCommands().deleteSubscriptions([item.id]));
          setBaseline({ name, url });
          await client.invalidateQueries();
          setBusy(false); busyRef.current = false; close();
        })().catch(setError).finally(() => { busyRef.current = false; setBusy(false); });
      } },
    ]);
  }
  return <DetailScreen>
    <TextField isInvalid={fields.name}><Label>{t("mobile.name")}</Label><Input accessibilityLabel={t("mobile.name")} value={name} onChangeText={setName} editable={!busy} /><FieldError>{t("mobile.nameRequired")}</FieldError></TextField>
    <TextField isInvalid={fields.url}><Label>{t("mobile.url")}</Label><Input accessibilityLabel={t("mobile.url")} value={url} onChangeText={setUrl} editable={!busy} autoCorrect={false} autoCapitalize="none" /><FieldError>{t("mobile.urlRequired")}</FieldError></TextField>
    <Typography className="text-sm text-subtle">{busy ? t("mobile.saving") : dirty ? t("mobile.unsaved") : t("mobile.saved")}</Typography>
    <Button isDisabled={!dirty || busy} onPress={() => void save()}><Button.Label>{t("actions.save")}</Button.Label></Button>
    <Typography className="text-base text-subtle">{t("nodeGroups.membersCount", { count: members.length })}</Typography>
    <Typography className="text-sm text-subtle">{metadata?.lastUpdateAt ? t("updates.lastUpdated", { time: new Date(metadata.lastUpdateAt * 1000).toLocaleString(language) }) : t("updates.neverUpdated")}</Typography>
    {metadata?.totalBytes != null ? <Typography className="text-sm text-subtle">{formatBytes((metadata.uploadBytes ?? 0) + (metadata.downloadBytes ?? 0))} / {formatBytes(metadata.totalBytes)}</Typography> : null}
    {metadata?.expireAt ? <Typography className="text-sm text-subtle">{new Date(metadata.expireAt * 1000).toLocaleDateString(language)}</Typography> : null}
    {message ? <Banner status={messageFailed ? "warning" : "info"} message={message} /> : null}
    <ErrorNotice error={error} message={dirty ? t("mobile.saveFailed") : undefined} />
    <Button variant="secondary" isDisabled={busy || dirty} onPress={() => void refresh()}><Button.Label>{t("home.subscriptionCard.update")}</Button.Label></Button>
    <Button variant="danger" isDisabled={busy || dirty || nodes.isPending || Boolean(nodes.error)} onPress={confirmDelete}><Button.Label>{t("actions.delete")}</Button.Label></Button>
  </DetailScreen>;
}
