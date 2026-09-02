import { Pencil, Play, Plus, Trash2 } from "lucide-react";

import { PageHeader } from "@/components/app-shell/page-section";
import { Badge } from "@voya/ui/components/badge";
import { Button } from "@voya/ui/components/button";
import { useI18n } from "@voya/i18n/use-i18n";

import type { RoutingScreenController } from "./use-routing-screen";

export function RoutingToolbar({ controller }: { controller: RoutingScreenController }) {
  const { t } = useI18n();
  const {
    activateSelectedRouting,
    deleteSelectedRouting,
    routings,
    selectedRouting,
    setRoutingDialog,
  } = controller;

  return (
    <PageHeader>
      <Badge variant="outline">{t("panes.routing.profileCount", { count: routings.length })}</Badge>

      <Button className="ms-auto" onClick={() => setRoutingDialog({ mode: "create" })} size="sm" type="button">
        <Plus className="size-4" aria-hidden="true" />
        {t("panes.routing.profile")}
      </Button>
      <Button
        disabled={!selectedRouting}
        onClick={() => selectedRouting && setRoutingDialog({ mode: "edit", routing: selectedRouting })}
        size="sm"
        type="button"
        variant="outline"
      >
        <Pencil className="size-4" aria-hidden="true" />
        {t("actions.edit")}
      </Button>
      <Button
        disabled={!selectedRouting || selectedRouting.isActive}
        onClick={activateSelectedRouting}
        size="sm"
        type="button"
        variant="outline"
      >
        <Play className="size-4" aria-hidden="true" />
        {t("actions.activate")}
      </Button>
      <Button
        disabled={!selectedRouting}
        onClick={deleteSelectedRouting}
        size="sm"
        type="button"
        variant="outline"
      >
        <Trash2 className="size-4" aria-hidden="true" />
        {t("actions.delete")}
      </Button>
    </PageHeader>
  );
}
