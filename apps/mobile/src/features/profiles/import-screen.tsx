import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { IpcCommandError } from "@voya/client/errors";
import { importLineText, isImportLineNotice } from "@voya/client/messages";
import { clipboard } from "@voya/client/platform";
import { refreshQueries } from "@voya/client/queries";
import { subscriptionRefreshRoots } from "@voya/client/query-keys";
import { voyaCommands } from "@voya/client/transport";
import type { ImportPreview } from "@voya/contracts";
import { formatImportSummary } from "@voya/features/profiles/server-table-actions";
import { getProtocolLabelLoose } from "@voya/features/profiles/profile-constants";
import { subscriptionUpdateMessages } from "@voya/features/subscriptions/subscription-update-result";
import { redactUrlQuery, urlHost } from "@voya/utils/redact-url-query";
import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "heroui-native/button";
import { Input } from "heroui-native/input";
import { Label } from "heroui-native/label";
import { TextField } from "heroui-native/text-field";
import { Typography } from "heroui-native/text";
import { Keyboard, Linking, View, useWindowDimensions } from "react-native";
import { navigateToTab } from "~/app/navigation";
import { Banner } from "~/components/banner";
import { DetailScreen } from "~/components/detail-screen";
import { ErrorNotice } from "~/components/error-notice";
import { PrimaryButton } from "~/components/primary-button";
import { useBusyAction } from "~/components/use-busy-action";
import { deviceActions } from "~/native/device-actions";

/**
 * Whether the backend refused the text because it holds nothing to import.
 * It reports unparseable text as a validation failure, and text that parses
 * but yields no node as "no profile found"; on this screen both mean the same
 * thing to the user, and the generic wording for a missing item
 * ("This item no longer exists") would be wrong.
 */
function nothingImportable(failure: unknown) {
  if (!(failure instanceof IpcCommandError)) return false;
  const { kind } = failure.appError;
  return kind.type === "validation" || (kind.type === "notFound" && kind.entity === "profile");
}

export function ImportScreen() {
  const { t } = useI18n();
  const client = useQueryClient();
  const { fontScale } = useWindowDimensions();
  const previewLimit = fontScale > 1.2 ? 1 : 3;
  const [text, setText] = useState("");
  const [preview, setPreview] = useState<{ text: string; data: ImportPreview } | null>(null);
  const [allNodes, setAllNodes] = useState(false);
  const [sourceNames, setSourceNames] = useState<Record<string, string>>({});
  const { busy, run: runOnce } = useBusyAction();
  const [error, setError] = useState<unknown>(null);
  const [errorMessage, setErrorMessage] = useState<string | undefined>();
  const [choices, setChoices] = useState<string[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [failedIds, setFailedIds] = useState<string[]>([]);
  const [importedCount, setImportedCount] = useState(0);
  const [denied, setDenied] = useState(false);
  function edit(value: string) { setText(value); setPreview(null); setAllNodes(false); setMessage(null); setError(null); setChoices([]); }
  const run = (action: () => Promise<void>) => runOnce(async () => {
    setError(null); setErrorMessage(undefined); setDenied(false);
    try { await action(); }
    catch (failure) {
      setError(failure);
      const code = failure && typeof failure === "object" && "code" in failure ? failure.code : null;
      setDenied(code === "cameraDenied");
      setErrorMessage(code === "cameraDenied" ? t("mobile.cameraDenied") : code === "noQr" ? t("mobile.noQr") : nothingImportable(failure) ? t("mobile.importInvalid") : undefined);
    }
  });
  async function recognize(camera: boolean) {
    const values = await (camera ? deviceActions().scanQr(t("actions.cancel")) : deviceActions().pickQr());
    if (!values) return;
    if (values.length === 1) edit(values[0]); else setChoices(values);
  }
  /** Downloads the given subscriptions; answers with the nodes they brought and what to say about the ones that did not. */
  async function updateSources(ids: string[]) {
    const failed: string[] = [];
    const notes: string[] = [];
    let nodes = 0;
    for (const id of ids) {
      try {
        const result = await voyaCommands().updateSubscriptions(id, true, null);
        if (result.outcomes.some((outcome) => outcome.status === "failed")) failed.push(id);
        nodes += result.imported + result.updated;
        const reason = subscriptionUpdateMessages(result, t);
        if (reason) notes.push(reason);
      } catch { failed.push(id); notes.push(t("mobile.updateFailed")); }
    }
    setFailedIds(failed);
    // Nothing was updated, so nothing went stale.
    if (ids.length) await refreshQueries(client, ...subscriptionRefreshRoots);

    return { nodes, notes };
  }
  async function retryFailed() {
    const retried = await updateSources(failedIds);
    setImportedCount((count) => count + retried.nodes);
    if (retried.notes.length) setMessage((value) => [value, ...retried.notes].filter(Boolean).join("\n"));
  }
  async function commit() {
    if (!preview || preview.text !== text) return;
    const result = await voyaCommands().importProfilesFromText(preview.text, null);
    const added = new Set(result.addedSubscriptionIds);
    // The import is committed by now. A name that cannot be applied leaves
    // the subscription under its default one, to be renamed from its own
    // page; letting that throw would call a finished import failed and skip
    // the first download below.
    try {
      for (const source of await voyaCommands().listSubscriptions()) {
        const name = sourceNames[source.url]?.trim();
        if (added.has(source.id) && name) await voyaCommands().saveSubscription({ ...source, remarks: name });
      }
    } catch { /* see above */ }
    setPreview(null); setText(""); setMessage(formatImportSummary(result, t));
    setImportedCount(result.imported);
    await refreshQueries(client, ...subscriptionRefreshRoots);
    // A subscription's nodes arrive with its first download, not with the
    // import that added it. They count: a page that said "Imported 0 nodes"
    // under a list that had just filled, and offered no way on to it, read as
    // a failed import.
    const downloaded = await updateSources(result.addedSubscriptionIds);
    if (downloaded.nodes > 0 || downloaded.notes.length > 0) {
      const imported = result.imported + downloaded.nodes;
      setImportedCount(imported);
      setMessage([formatImportSummary({ ...result, imported }, t), ...downloaded.notes].join("\n"));
    }
  }
  return <DetailScreen key={preview ? "preview" : message ? "summary" : "input"}>
    <ErrorNotice error={error} message={errorMessage} />
    {!preview && !message ? <>
    <Typography className="text-base text-subtle">{t("mobile.importHelp")}</Typography>
    <Input multiline scrollEnabled style={{ maxHeight: 240 }} className="min-h-32 h-auto" accessibilityLabel={t("mobile.add")} placeholder={t("panes.profiles.importDialog.placeholder")} value={text} onChangeText={edit} editable={!busy} autoCapitalize="none" autoCorrect={false} />
    <View className="flex-row flex-wrap gap-2">
      <Button variant="secondary" isDisabled={busy} onPress={() => void run(async () => edit(await clipboard().readText()))}><Button.Label>{t("mobile.paste")}</Button.Label></Button>
      <Button variant="secondary" isDisabled={busy} onPress={() => void run(() => recognize(true))}><Button.Label>{t("mobile.scan")}</Button.Label></Button>
      <Button variant="secondary" isDisabled={busy} onPress={() => void run(() => recognize(false))}><Button.Label>{t("mobile.image")}</Button.Label></Button>
    </View>
    {choices.length ? <><Typography className="text-base text-foreground">{t("mobile.chooseQr")}</Typography>{choices.map((value, index) => <Button key={index} variant="secondary" onPress={() => edit(value)}><Button.Label numberOfLines={2}>{value}</Button.Label></Button>)}</> : null}
    {denied ? <Button variant="secondary" onPress={() => void Linking.openSettings()}><Button.Label>{t("tabs.settings")}</Button.Label></Button> : null}
    <PrimaryButton label={t("mobile.preview")} isDisabled={busy || !text.trim()} onPress={() => void run(async () => { const data = await voyaCommands().previewImportProfiles(text); Keyboard.dismiss(); setMessage(null); setPreview({ text, data }); })} />
    </> : null}
    {preview ? <>
      <Typography className="text-base text-foreground">{t("mobile.previewSummary", {
        nodes: t("mobile.previewNodes", { count: preview.data.nodes.length }),
        subscriptions: t("mobile.previewSubscriptions", { count: preview.data.subscriptionUrls.length }),
        invalid: t("mobile.previewInvalid", { count: preview.data.failed }),
      })}</Typography>
      {preview.data.lineIssues.map((issue, index) => <Typography key={`issue-${index}`} className={isImportLineNotice(issue) ? "text-sm text-subtle" : "text-sm text-danger"}>{importLineText(t, issue)}</Typography>)}
      {(allNodes ? preview.data.nodes : preview.data.nodes.slice(0, previewLimit)).map((node, index) => <Typography key={index} className="text-base text-foreground">{node.name} · {getProtocolLabelLoose(node.protocol)} · {node.address}</Typography>)}
      {preview.data.nodes.length > previewLimit ? <Button variant="secondary" accessibilityState={{ expanded: allNodes }} onPress={() => setAllNodes(!allNodes)}><Button.Label>{allNodes ? t("mobile.collapsePreview") : t("mobile.expandPreview", { count: preview.data.nodes.length })}</Button.Label></Button> : null}
      {preview.data.subscriptionUrls.map((url) => <View key={url} className="gap-2">
        {/* The token never prints: the value is masked, the host and path are
            what identify the source. */}
        <Typography selectable className="text-base text-foreground">{urlHost(url)}{"\n"}{redactUrlQuery(url)}</Typography>
        <TextField><Label>{t("mobile.name")}</Label><Input accessibilityLabel={t("mobile.name")} placeholder={t("mobile.name")} value={sourceNames[url] ?? ""} editable={!busy} onChangeText={(value) => setSourceNames((previous) => ({ ...previous, [url]: value }))} /></TextField>
      </View>)}
      <Button variant="secondary" isDisabled={busy} onPress={() => { setPreview(null); setAllNodes(false); }}><Button.Label>{t("actions.edit")}</Button.Label></Button>
      <PrimaryButton label={t("mobile.confirmImport")} isDisabled={busy || !(preview.data.nodes.length || preview.data.subscriptionUrls.length)} onPress={() => void run(commit)} />
    </> : null}
    {message ? <>
      <Banner status={failedIds.length ? "warning" : "info"} message={message} liveRegion />
      {/* One primary action, by what the user most plausibly does next: retry
          what failed, go pick from what imported, or import something else. */}
      {failedIds.length ? <PrimaryButton label={t("mobile.retryFailed")} isDisabled={busy} onPress={() => void run(retryFailed)} /> : null}
      {importedCount > 0 ? <Button onPress={() => navigateToTab("profiles")}><Button.Label>{t("home.chooseNode")}</Button.Label></Button> : null}
      <Button variant={failedIds.length || importedCount > 0 ? "secondary" : undefined} isDisabled={busy} className={busy && !(failedIds.length || importedCount > 0) ? "button--primary-disabled" : undefined} onPress={() => { setMessage(null); setFailedIds([]); }}>
        <Button.Label className={busy && !(failedIds.length || importedCount > 0) ? "text-subtle" : undefined}>{t("mobile.add")}</Button.Label>
      </Button>
    </> : null}
  </DetailScreen>;
}
