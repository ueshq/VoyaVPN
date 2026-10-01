import type { NativeStackScreenProps } from "@react-navigation/native-stack";
import { clipboard } from "@voya/client/platform";
import { useI18n } from "@voya/i18n/use-i18n";
import { ruleDisplayName, SENTINEL_BLOCK_QUIC } from "@voya/features/routing/sentinel-rules";
import { Button } from "heroui-native/button";
import { Typography } from "heroui-native/text";
import type { RootRoutes } from "~/app/navigation";
import { DetailScreen } from "~/components/detail-screen";

export function RuleDetailsScreen({ route }: NativeStackScreenProps<RootRoutes, "ruleDetails">) {
  const { t } = useI18n();
  const { rule, target } = route.params;
  return <DetailScreen>
    <Typography className="text-xl font-semibold text-foreground">{ruleDisplayName(rule, t)}</Typography>
    <Typography className="text-base text-subtle">{t("mobile.ruleEffect", { target })}</Typography>
    {rule.remarks === SENTINEL_BLOCK_QUIC ? <Typography className="text-sm text-subtle">{t("mobile.quicHint")}</Typography> : null}
    <Typography selectable className="text-base text-foreground">{JSON.stringify(rule, null, 2)}</Typography>
    {/* Like the log page's copy action: no toast, the clipboard just has it. */}
    <Button variant="secondary" onPress={() => void clipboard().writeText(JSON.stringify(rule, null, 2)).catch(() => {})}>
      <Button.Label>{t("mobile.copyJson")}</Button.Label>
    </Button>
  </DetailScreen>;
}
