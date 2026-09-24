import { outboundLabelKey } from "@voya/features/routing/rule-outbound";
import type { NativeStackScreenProps } from "@react-navigation/native-stack";
import { useI18n } from "@voya/i18n/use-i18n";
import { ruleDisplayName } from "@voya/features/routing/sentinel-rules";
import { Typography } from "heroui-native/text";
import type { RootRoutes } from "~/app/navigation";
import { DetailScreen } from "~/components/detail-screen";

export function RuleDetailsScreen({ route }: NativeStackScreenProps<RootRoutes, "ruleDetails">) {
  const { t } = useI18n();
  const { rule } = route.params;
  const outboundKey = outboundLabelKey(rule.outbound ?? "proxy");
  return <DetailScreen>
    <Typography className="text-xl font-semibold text-foreground">{ruleDisplayName(rule, t)}</Typography>
    <Typography className="text-base text-subtle">{t("mobile.ruleEffect", { target: outboundKey ? t(outboundKey) : rule.outbound })}</Typography>
    {rule.remarks === "voya:block-quic" ? <Typography className="text-sm text-subtle">{t("mobile.quicHint")}</Typography> : null}
    <Typography selectable className="text-base text-foreground">{JSON.stringify(rule, null, 2)}</Typography>
  </DetailScreen>;
}
