import { useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { IpcCommandError } from "@voya/client/errors";
import { importLineText } from "@voya/client/messages";
import { clipboard } from "@voya/client/platform";
import { voyaCommands } from "@voya/client/transport";
import type { ImportPreview } from "@voya/contracts";
import { formatImportSummary } from "@voya/features/profiles/server-table-actions";
import { subscriptionUpdateMessages } from "@voya/features/subscriptions/subscription-update-result";
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
import { scanQr, pickQr } from "~/native/device-actions";

export function ImportScreen() {
  const { t } = useI18n();
  const client = useQueryClient();
  const { fontScale } = useWindowDimensions();
  const previewLimit = fontScale > 1.2 ? 1 : 3;
  const [text, setText] = useState("");
  const [preview, setPreview] = useState<{ text: string; data: ImportPreview } | null>(null);
  const [allNodes, setAllNodes] = useState(false);
  const [sourceNames, setSourceNames] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const [error, setError] = useState<unknown>(null);
  const [errorMessage, setErrorMessage] = useState<string | undefined>();
  const [choices, setChoices] = useState<string[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [failedIds, setFailedIds] = useState<string[]>([]);
  const [denied, setDenied] = useState(false);
  function edit(value: string) { setText(value); setPreview(null); setAllNodes(false); setMessage(null); setError(null); setChoices([]); }
  async function run(action: () => Promise<void>) {
    if (busyRef.current) return;
    busyRef.current = true; setBusy(true); setError(null); setErrorMessage(undefined); setDenied(false);
    try { await action(); }
    catch (failure) {
      setError(failure);
      const code = failure && typeof failure === "object" && "code" in failure ? failure.code : null;
      setDenied(code === "cameraDenied");
      setErrorMessage(code === "cameraDenied" ? t("mobile.cameraDenied") : code === "noQr" ? t("mobile.noQr") : failure instanceof IpcCommandError && failure.appError.kind.type === "validation" ? t("mobile.importInvalid") : undefined);
    } finally { busyRef.current = false; setBusy(false); }
  }
  async function recognize(camera: boolean) {
    const values = await (camera ? scanQr(t("actions.cancel")) : pickQr());
    if (!values) return;
    if (values.length === 1) edit(values[0]); else setChoices(values);
  }
  async function updateSources(ids: string[]) {
    const failed: string[] = [];
    const notes: string[] = [];
    for (const id of ids) {
      try {
        const result = await voyaCommands().updateSubscriptions(id, true, null);
        if (result.outcomes.some((outcome) => outcome.status === "failed")) failed.push(id);
        const reason = subscriptionUpdateMessages(result, t);
        if (reason) notes.push(reason);
      } catch { failed.push(id); notes.push(t("mobile.updateFailed")); }
    }
    setFailedIds(failed);
    if (notes.length) setMessage((value) => [value, ...notes].filter(Boolean).join("\n"));
    await client.invalidateQueries();
  }
  async function commit() {
    if (!preview || preview.text !== text) return;
    const result = await voyaCommands().importProfilesFromText(preview.text, null);
    const added = new Set(result.addedSubscriptionIds);
    for (const source of await voyaCommands().listSubscriptions()) {
      const name = sourceNames[source.url]?.trim();
      if (added.has(source.id) && name) await voyaCommands().saveSubscription({ ...source, remarks: name });
    }
    setPreview(null); setText(""); setMessage(formatImportSummary(result, t));
    await client.invalidateQueries();
    await updateSources(result.addedSubscriptionIds);
  }
  return <DetailScreen key={preview ? "preview" : "input"}>
    <ErrorNotice error={error} message={errorMessage} />
    {!preview ? <>
    <Typography className="text-base text-subtle">{t("mobile.importHelp")}</Typography>
    <Input multiline scrollEnabled style={{ maxHeight: 240 }} className="min-h-32 h-auto" accessibilityLabel={t("mobile.add")} value={text} onChangeText={edit} editable={!busy} autoCapitalize="none" autoCorrect={false} />
    <View className="flex-row flex-wrap gap-2">
      <Button variant="secondary" className="min-h-12 h-auto" isDisabled={busy} onPress={() => void run(async () => edit(await clipboard().readText()))}><Button.Label>{t("mobile.paste")}</Button.Label></Button>
      <Button variant="secondary" className="min-h-12 h-auto" isDisabled={busy} onPress={() => void run(() => recognize(true))}><Button.Label>{t("mobile.scan")}</Button.Label></Button>
      <Button variant="secondary" className="min-h-12 h-auto" isDisabled={busy} onPress={() => void run(() => recognize(false))}><Button.Label>{t("mobile.image")}</Button.Label></Button>
    </View>
    {choices.length ? <><Typography className="text-base text-foreground">{t("mobile.chooseQr")}</Typography>{choices.map((value, index) => <Button key={index} className="min-h-12 h-auto" variant="secondary" onPress={() => edit(value)}><Button.Label numberOfLines={2}>{value}</Button.Label></Button>)}</> : null}
    {denied ? <Button variant="secondary" className="min-h-12 h-auto" onPress={() => void Linking.openSettings()}><Button.Label>{t("tabs.settings")}</Button.Label></Button> : null}
    <Button className="min-h-12 h-auto" isDisabled={busy || !text.trim()} onPress={() => void run(async () => { const data = await voyaCommands().previewImportProfiles(text); Keyboard.dismiss(); setPreview({ text, data }); })}><Button.Label>{t("mobile.preview")}</Button.Label></Button>
    </> : null}
    {preview ? <>
      <Typography className="text-base text-foreground">{t("mobile.previewCount", { nodes: preview.data.nodes.length, sources: preview.data.subscriptionUrls.length, failed: preview.data.failed })}</Typography>
      {preview.data.lineIssues.map((issue, index) => <Typography key={`issue-${index}`} className="text-sm text-danger">{importLineText(t, issue)}</Typography>)}
      {(allNodes ? preview.data.nodes : preview.data.nodes.slice(0, previewLimit)).map((node, index) => <Typography key={index} className="text-base text-foreground">{node.name} · {node.protocol} · {node.address}</Typography>)}
      {preview.data.nodes.length > previewLimit ? <Button variant="secondary" className="min-h-12 h-auto" accessibilityState={{ expanded: allNodes }} onPress={() => setAllNodes(!allNodes)}><Button.Label>{allNodes ? t("mobile.collapsePreview") : t("mobile.expandPreview", { count: preview.data.nodes.length })}</Button.Label></Button> : null}
      {preview.data.subscriptionUrls.map((url) => <View key={url} className="gap-2">
        <Typography selectable className="text-base text-foreground">{new URL(url).hostname}{"\n"}{url}</Typography>
        <TextField><Label>{t("mobile.name")}</Label><Input accessibilityLabel={t("mobile.name")} placeholder={t("mobile.name")} value={sourceNames[url] ?? ""} editable={!busy} onChangeText={(value) => setSourceNames((previous) => ({ ...previous, [url]: value }))} /></TextField>
      </View>)}
      <Button variant="secondary" className="min-h-12 h-auto" isDisabled={busy} onPress={() => { setPreview(null); setAllNodes(false); }}><Button.Label>{t("actions.edit")}</Button.Label></Button>
      <Button className="min-h-12 h-auto" isDisabled={busy || !(preview.data.nodes.length || preview.data.subscriptionUrls.length)} onPress={() => void run(commit)}><Button.Label>{t("mobile.confirmImport")}</Button.Label></Button>
    </> : null}
    {message ? <Banner status={failedIds.length ? "warning" : "info"} message={message} liveRegion /> : null}
    {failedIds.length ? <Button variant="secondary" className="min-h-12 h-auto" isDisabled={busy} onPress={() => void run(() => updateSources(failedIds))}><Button.Label>{t("mobile.retryFailed")}</Button.Label></Button> : null}
    {message ? <Button variant="secondary" className="min-h-12 h-auto" onPress={() => navigateToTab("profiles")}><Button.Label>{t("mobile.select")}</Button.Label></Button> : null}
  </DetailScreen>;
}
