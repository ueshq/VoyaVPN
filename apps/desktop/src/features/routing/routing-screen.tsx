import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@voya/ui/components/select";
import { TriangleAlert } from "lucide-react";

import { PageSection, PageTitle } from "@/components/app-shell/page-section";
import { Alert, AlertDescription } from "@voya/ui/components/alert";
import { useI18n } from "@voya/i18n/use-i18n";

import { RoutingDialogs } from "./routing-dialogs";
import { RoutingProfileList } from "./routing-profile-list";
import { RoutingRulesPanel } from "./routing-rules-panel";
import { RoutingToolbar } from "./routing-toolbar";
import { useRoutingScreen } from "./use-routing-screen";

export function RoutingScreen() {
  const { t } = useI18n();
  const controller = useRoutingScreen();

  return (
    <PageSection className="@container/routing" aria-label={t("tabs.rules")}>
      <PageTitle title={t("tabs.rules")} />
      <RoutingToolbar controller={controller} />

      {controller.operationError ? (
        <div className="border-b px-4 py-2">
          <Alert className="py-2" variant="destructive">
            <TriangleAlert aria-hidden="true" />
            <AlertDescription>{controller.operationError}</AlertDescription>
          </Alert>
        </div>
      ) : null}

      <div className="border-b p-4 @min-[896px]/routing:hidden">
        <Select
          value={controller.selectedRouting?.id ?? ""}
          onValueChange={controller.selectRouting}
        >
          <SelectTrigger
            className="w-full"
            aria-label={t("panes.routing.chooseProfile")}
          >
            <SelectValue placeholder={t("panes.routing.chooseProfile")} />
          </SelectTrigger>
          <SelectContent>
            {controller.routings.map((routing) => (
              <SelectItem key={routing.id} value={routing.id}>
                {routing.remarks || t("panes.routing.untitled")}
                {routing.isActive ? ` · ${t("panes.routing.active")}` : ""}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <div className="grid min-h-0 flex-1 grid-cols-1 @min-[896px]/routing:grid-cols-[18rem_minmax(0,1fr)]">
        <div className="hidden min-h-0 @min-[896px]/routing:flex">
          <RoutingProfileList controller={controller} />
        </div>
        <RoutingRulesPanel controller={controller} />
      </div>

      <RoutingDialogs controller={controller} />
    </PageSection>
  );
}
