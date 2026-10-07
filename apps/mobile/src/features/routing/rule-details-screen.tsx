import { ruleMatchSummary } from "@voya/features/routing/rule-summary";
import { useSavedTrafficMode } from "@voya/features/routing/use-traffic-mode";
import { Disclosure } from "~/components/disclosure";
import type { NativeStackScreenProps } from "@react-navigation/native-stack";
import { clipboard } from "@voya/client/platform";
import { useI18n } from "@voya/i18n/use-i18n";
import { ruleDisplayName, SENTINEL_BLOCK_QUIC } from "@voya/features/routing/sentinel-rules";
import type { RoutingRule } from "@voya/contracts";
import { Button } from "heroui-native/button";
import { Typography } from "heroui-native/text";
import { View, ScrollView } from "react-native";
import type { RootRoutes } from "~/app/navigation";
import { DetailScreen } from "~/components/detail-screen";
import { EdgeFade } from "~/components/edge-fade";
import { MONO_FONT } from "~/components/mono-font";
import { useCopiedLabel } from "~/components/use-copied-label";

/** How far a long line dissolves into the card's right edge. */
const FADE_WIDTH = 16;

/**
 * Fields the editor has no control over: the rule's database identity, and
 * the inbound tags and rule kind that the desktop's `routing-form-values`
 * carries through a save untouched. They explain storage, not behaviour, so
 * the page leaves them out of what it shows and what it copies.
 */
const INTERNAL_FIELDS = new Set(["id", "inboundTags", "kind"]);

/** The JSON the page shows and copies: what the rule does, nothing else. */
function displayRuleJson(rule: RoutingRule): string {
  const shown = Object.fromEntries(
    Object.entries(rule).filter(
      ([key, value]) => value !== null && value !== undefined && !INTERNAL_FIELDS.has(key),
    ),
  );
  return JSON.stringify(shown, null, 2);
}

export function RuleDetailsScreen({ route }: NativeStackScreenProps<RootRoutes, "ruleDetails">) {
  const { t } = useI18n();
  const { rule, target } = route.params;
  const json = displayRuleJson(rule);
  const mode = useSavedTrafficMode();
  const scopeLabel = rule.scope === "dns" ? "daily.ruleScopeDns" : rule.scope === "routing" ? "daily.ruleScopeRouting" : "daily.ruleScopeAll";
  const { copied, failed, markCopied, markFailed } = useCopiedLabel();
  // Nested ternary kept out of the `t()` call: the i18n check reads one
  // conditional level, not two.
  const copyLabelKey = copied
    ? "mobile.copied"
    : failed
      ? "mobile.copyFailed"
      : "mobile.copyJson";
  return <DetailScreen>
    <Typography className="text-xl font-semibold text-foreground">{ruleDisplayName(rule, t)}</Typography>
    <Typography className="text-base text-subtle">{t("mobile.ruleEffect", { target })}</Typography>
    {rule.remarks === SENTINEL_BLOCK_QUIC ? <Typography className="text-sm text-subtle">{t("mobile.quicHint")}</Typography> : null}
    {!rule.enabled ? <Typography className="text-base text-subtle">{t("daily.ruleDisabled")}</Typography> : null}
    {mode.mode === "global" && rule.scope !== "dns" ? <Typography className="text-base text-subtle">{t("daily.globalRuleInactive")}</Typography> : null}
    <Typography className="text-base text-subtle">{t(scopeLabel)}</Typography>
    <Typography accessibilityRole="header" className="text-lg font-semibold text-foreground">{t("daily.ruleConditions")}</Typography>
    {ruleMatchSummary(rule, t).map((line) => <Typography key={line} selectable className="text-base text-foreground">{line}</Typography>)}
    {rule.process?.length ? <Typography className="text-base text-warning">{t("panes.routing.processUnsupported")}</Typography> : null}
    <Disclosure title={t("mobile.details")}>
    {/* Raw JSON on a card in a mono face, so structure reads as code rather
        than as prose that happens to have braces in it. One Text per line,
        each refusing to wrap, inside a horizontal panner: a wrapping Text
        breaks a long id or domain flush to the margin and the indentation it
        belonged to is gone, while the panner keeps every line intact and
        scrolls to what overflows — the code-view convention. The fade says
        so: a clipped edge reads broken, a dissolving one reads "more". */}
    <View>
      <ScrollView horizontal className="rounded-2xl bg-surface" contentContainerClassName="p-4">
        <View>
          {json.split("\n").map((line, index) => (
            <Typography selectable key={index} style={{ fontFamily: MONO_FONT }} className="text-sm text-foreground" numberOfLines={1}>{line || " "}</Typography>
          ))}
        </View>
      </ScrollView>
      {/* The card's right edge dissolving into itself, signalling the scroll. */}
      <EdgeFade
        edge="right"
        size={FADE_WIDTH}
        colorClassName="bg-surface"
        className="absolute inset-y-0 right-0 w-4 overflow-hidden rounded-r-2xl"
      />
    </View>
    {/* The label itself is the confirmation: it says "Copied" for a beat,
        then goes back to inviting the next tap — and "Copy failed" for the
        same beat when the clipboard refused, so the tap is never lost
        silently. */}
    <Button variant="secondary" onPress={() => void clipboard().writeText(json).then(markCopied).catch(markFailed)}>
      <Button.Label>{t(copyLabelKey)}</Button.Label>
    </Button>
    </Disclosure>
  </DetailScreen>;
}
