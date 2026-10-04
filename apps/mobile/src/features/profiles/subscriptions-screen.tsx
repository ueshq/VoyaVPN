import { queries, refreshQueries } from "@voya/client/queries";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import type { NativeStackScreenProps } from "@react-navigation/native-stack";
import { voyaCommands } from "@voya/client/transport";
import { queryKeys, subscriptionRefreshRoots } from "@voya/client/query-keys";
import type { Subscription, SubscriptionMetadata } from "@voya/contracts";
import { useI18n } from "@voya/i18n/use-i18n";
import { formatDateTime } from "@voya/utils/format-date-time";
import { isSubscriptionUrl } from "@voya/features/subscriptions/subscriptions-form-schema";
import { subscriptionUsageStats } from "@voya/features/subscriptions/subscription-usage";
import { formatSubscriptionUpdateSummary, subscriptionUpdateMessages } from "@voya/features/subscriptions/subscription-update-result";
import { Button } from "heroui-native/button";
import { FieldError } from "heroui-native/field-error";
import { Input } from "heroui-native/input";
import { Label } from "heroui-native/label";
import { ListGroup } from "heroui-native/list-group";
import { TextField } from "heroui-native/text-field";
import { Typography } from "heroui-native/text";
import { Rss } from "lucide-react-native";
import { useEffect, useRef, useState } from "react";
import { Alert } from "react-native";
import { openPage, type RootRoutes } from "~/app/navigation";
import { DetailScreen } from "~/components/detail-screen";
import { Banner } from "~/components/banner";
import { Disclosure } from "~/components/disclosure";
import { EmptyState } from "~/components/empty-state";
import { ErrorNotice } from "~/components/error-notice";
import { ListRow } from "~/components/list-row";
import { useBusyAction } from "~/components/use-busy-action";
import { useUnsavedChanges } from "~/components/use-unsaved-changes";
import { deleteSafely } from "./delete-safely";
import { SaveStatus } from "~/components/save-status";
import { PrimaryButton } from "~/components/primary-button";

function useSources() {
  const sources = useQuery(queries.subscriptions);
  const metadata = useQuery(queries.subscriptionMetadata);
  return { sources, metadata };
}

export function SubscriptionsScreen() {
  const { t, language } = useI18n();
  const { sources, metadata } = useSources();
  const client = useQueryClient();
  const { busy, run } = useBusyAction();
  const [failedIds, setFailedIds] = useState<string[]>([]);
  const [error, setError] = useState<unknown>(null);
  const [message, setMessage] = useState<string | null>(null);
  const update = (ids: string[] | null) => run(async () => {
    setError(null); setMessage(null);
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
      await refreshQueries(client, ...subscriptionRefreshRoots);
    } finally { setFailedIds([...failed]); }
  });
  return <DetailScreen>
    <Button onPress={() => openPage("import")}><Button.Label>{t("mobile.add")}</Button.Label></Button>
    <Button variant="secondary" isDisabled={busy || !sources.data?.length} onPress={() => void update(null)}><Button.Label>{t("panes.profiles.toolbar.updateAllSubscriptions")}</Button.Label></Button>
    {failedIds.length ? <Button variant="secondary" isDisabled={busy} onPress={() => void update(failedIds)}><Button.Label>{t("mobile.retryFailed")}</Button.Label></Button> : null}
    {message ? <Banner status={failedIds.length ? "warning" : "info"} liveRegion message={message} /> : null}
    <ErrorNotice error={error ?? sources.error} retry={() => void sources.refetch()} />
    {/* The persisted attempt outlives this screen — a subscription that failed
        on an earlier launch keeps its badge until an update succeeds. */}
    {!sources.isPending && !sources.error && !sources.data?.length ? (
      <EmptyState
        icons={[Rss]}
        title={t("mobile.subsEmptyTitle")}
        description={t("mobile.subsEmptyDescription")}
      />
    ) : null}
    <ListGroup>{sources.data?.map((item, index, all) => <ListRow key={item.id} title={item.remarks}
      description={rowDescription(item.id, failedIds, metadata.data, t, language)} descriptionLines={0}
      last={index === all.length - 1} chevron onPress={() => openPage("subscription", { id: item.id })} />)}</ListGroup>
  </DetailScreen>;
}

function rowDescription(
  id: string,
  failedIds: string[],
  metadata: SubscriptionMetadata[] | undefined,
  t: ReturnType<typeof useI18n>["t"],
  language: string,
) {
  if (failedIds.includes(id)) return t("mobile.updateFailed");
  const failed = metadata?.find((meta) => meta.subscriptionId === id && meta.lastAttemptFailed);
  if (failed?.lastAttemptAt) {
    return t("mobile.updateFailedAt", {
      time: formatDateTime(failed.lastAttemptAt * 1000, language),
    });
  }

  return undefined;
}

export function SubscriptionScreen({ route, navigation }: NativeStackScreenProps<RootRoutes, "subscription">) {
  const { t } = useI18n();
  const { sources, metadata } = useSources();
  const item = sources.data?.find((source) => source.id === route.params.id);
  // Leaving after a delete is this screen's to do, not the editor's: the
  // editor unmounts as soon as the refreshed list no longer has its
  // subscription, and while it is still mounted and busy its unsaved-changes
  // guard refuses to let the page go. An effect runs after both have settled.
  const [deleted, setDeleted] = useState(false);
  const left = useRef(false);
  useEffect(() => {
    if (deleted && !left.current) {
      left.current = true;
      navigation.goBack();
    }
  }, [deleted, navigation]);
  if (deleted) return <DetailScreen>{null}</DetailScreen>;
  return item ? <SubscriptionEditor key={item.id} item={item} metadata={metadata.data?.find((meta) => meta.subscriptionId === item.id)} onDeleted={() => setDeleted(true)} />
    : <DetailScreen><ErrorNotice error={sources.error} retry={() => void sources.refetch()} />{!sources.error ? <Typography className="text-base text-subtle">{sources.isPending ? t("options.loading") : t("mobile.sourceMissing")}</Typography> : null}</DetailScreen>;
}

function SubscriptionEditor({ item, metadata, onDeleted }: { item: Subscription; metadata?: SubscriptionMetadata; onDeleted: () => void }) {
  const { t, language } = useI18n();
  const client = useQueryClient();
  const [name, setName] = useState(item.remarks);
  const [url, setUrl] = useState(item.url);
  const [baseline, setBaseline] = useState({ name, url });
  const { busy, run } = useBusyAction();
  const [error, setError] = useState<unknown>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [messageFailed, setMessageFailed] = useState(false);
  const [fields, setFields] = useState({ name: false, url: false });
  const dirty = name !== baseline.name || url !== baseline.url;
  const nodes = useQuery(queries.profileList);
  const members = nodes.data?.entries.filter((entry) => entry.profile.subscriptionId === item.id) ?? [];
  const save = async () => (await run(saveEdits)) ?? false;
  async function saveEdits() {
    // The shared rule, not `new URL`: React Native's accepts nearly anything
    // without throwing, so a check built on it passes in a test and not on a
    // device.
    const validUrl = isSubscriptionUrl(url);
    setFields({ name: !name.trim(), url: !validUrl });
    if (!name.trim() || !validUrl) { setError(t("mobile.saveFailed")); return false; }
    setError(null);
    try {
      const current = (await voyaCommands().listSubscriptions()).find((source) => source.id === item.id);
      if (!current) throw new Error("Subscription no longer exists");
      await voyaCommands().saveSubscription({ ...current, remarks: name.trim(), url: url.trim() });
      setBaseline({ name, url }); setMessageFailed(false); setMessage(t("mobile.saved"));
      await client.invalidateQueries({ queryKey: queryKeys.subscriptions });
      return true;
    } catch (failure) { setError(failure); return false; }
  }
  useUnsavedChanges(dirty, busy, save);
  const refresh = () => run(async () => {
    if (dirty) return;
    setError(null);
    try {
      const result = await voyaCommands().updateSubscriptions(item.id, true, null);
      const notes = subscriptionUpdateMessages(result, t);
      // Only a failed download is a warning. A skipped one means the
      // subscription was edited while it downloaded; the note says to refresh
      // again, and nothing went wrong.
      setMessageFailed(result.outcomes.some((outcome) => outcome.status === "failed"));
      setMessage(notes || formatSubscriptionUpdateSummary(result, t));
      await refreshQueries(client, ...subscriptionRefreshRoots);
    } catch (failure) { setError(failure); }
  });
  function confirmDelete() {
    Alert.alert(t("mobile.deleteConfirm"), t("mobile.deleteDescription", { count: members.length }), [
      { text: t("actions.cancel"), style: "cancel" },
      { text: t("actions.delete"), style: "destructive", onPress: () => {
        void run(async () => {
          const currentNodes = await voyaCommands().listProfileSummaries();
          await deleteSafely(currentNodes.entries.filter((entry) => entry.profile.subscriptionId === item.id).map((entry) => entry.profile.id), () => voyaCommands().deleteSubscriptions([item.id]));
          setBaseline({ name, url });
          await refreshQueries(client, ...subscriptionRefreshRoots);
          return true;
        }).then((done) => { if (done) onDeleted(); }, setError);
      } },
    ]);
  }
  return <DetailScreen>
    <TextField isInvalid={fields.name}><Label>{t("mobile.name")}</Label><Input accessibilityLabel={t("mobile.name")} value={name} onChangeText={setName} editable={!busy} /><FieldError>{t("subscriptions.validation.name")}</FieldError></TextField>
    <TextField isInvalid={fields.url}><Label>{t("mobile.url")}</Label><Input accessibilityLabel={t("mobile.url")} value={url} onChangeText={setUrl} editable={!busy} autoCorrect={false} autoCapitalize="none" /><FieldError>{t("subscriptions.validation.url")}</FieldError></TextField>
    {/* The banner below says "Saved" after a save; a status line that says it
        before anything happened teaches the user to ignore both. */}
    <SaveStatus dirty={dirty} saving={busy} />
    <PrimaryButton label={t("actions.save")} isDisabled={!dirty || busy} onPress={() => void save()} />
    <Typography className="text-base text-subtle">{t("nodeGroups.membersCount", { count: members.length })}</Typography>
    {/* A failed attempt is the more recent fact than any successful update,
        so it replaces the status line instead of hiding behind it. */}
    {metadata?.lastAttemptFailed ? <>
      <Typography className="text-sm text-warning">
        {metadata.lastAttemptAt
          ? t("mobile.updateFailedAt", { time: formatDateTime(metadata.lastAttemptAt * 1000, language) })
          : t("mobile.updateFailed")}
      </Typography>
      {metadata.lastAttemptError ? (
        <Disclosure title={t("mobile.details")}>
          <Typography className="text-sm text-subtle" selectable>{metadata.lastAttemptError}</Typography>
        </Disclosure>
      ) : null}
    </> : (
      <Typography className="text-sm text-subtle">{metadata?.lastUpdateAt ? t("updates.lastUpdated", { time: formatDateTime(metadata.lastUpdateAt * 1000, language) }) : t("updates.neverUpdated")}</Typography>
    )}
    {/* The same two facts, in the same words, as the desktop's card: what is
        left of the quota and of the term, each only when the server says. */}
    {subscriptionUsageStats(metadata, t).map((stat) => (
      <Typography key={stat.key} className={stat.destructive ? "text-sm text-danger" : "text-sm text-subtle"}>{stat.label}</Typography>
    ))}
    {message ? <Banner status={messageFailed ? "warning" : "info"} message={message} /> : null}
    <ErrorNotice error={error} message={dirty ? t("mobile.saveFailed") : undefined} />
    <Button variant="secondary" isDisabled={busy || dirty} onPress={() => void refresh()}><Button.Label>{t("home.subscriptionCard.update")}</Button.Label></Button>
    <Button variant="danger" isDisabled={busy || dirty || nodes.isPending || Boolean(nodes.error)} onPress={confirmDelete}><Button.Label>{t("actions.delete")}</Button.Label></Button>
  </DetailScreen>;
}
