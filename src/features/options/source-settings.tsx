import { useEffect, useState } from "react";
import { Globe2, Save } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useI18n } from "@/i18n/use-i18n";
import { loadRulesetGeoSources, saveRulesetGeoSources } from "@/ipc";

type SourceForm = {
  geoSourceUrl: string;
  srsSourceUrl: string;
};

const emptyForm: SourceForm = {
  geoSourceUrl: "",
  srsSourceUrl: "",
};

export function SourceSettings() {
  const { t } = useI18n();
  const [error, setError] = useState<string | null>(null);
  const [form, setForm] = useState<SourceForm>(emptyForm);
  const [saved, setSaved] = useState(false);
  const [working, setWorking] = useState(false);

  useEffect(() => {
    let disposed = false;

    void loadRulesetGeoSources()
      .then((settings) => {
        if (disposed) {
          return;
        }
        setForm({
          geoSourceUrl: settings.geoSourceUrl ?? "",
          srsSourceUrl: settings.srsSourceUrl ?? "",
        });
      })
      .catch((error: unknown) => {
        if (!disposed) {
          setError(error instanceof Error ? error.message : String(error));
        }
      });

    return () => {
      disposed = true;
    };
  }, []);

  async function save() {
    setWorking(true);
    setError(null);
    setSaved(false);
    try {
      const settings = await saveRulesetGeoSources({
        geoSourceUrl: form.geoSourceUrl.trim() || null,
        srsSourceUrl: form.srsSourceUrl.trim() || null,
      });
      setForm({
        geoSourceUrl: settings.geoSourceUrl ?? "",
        srsSourceUrl: settings.srsSourceUrl ?? "",
      });
      setSaved(true);
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
    } finally {
      setWorking(false);
    }
  }

  return (
    <section className="grid gap-3">
      <h3 className="flex items-center gap-2 text-sm font-medium">
        <Globe2 className="size-4" aria-hidden="true" />
        {t("options.sources")}
      </h3>

      <div className="grid gap-1.5">
        <Label className="text-muted-foreground" htmlFor="ruleset-geo-source-url">
          {t("options.geoSource")}
        </Label>
        <Input
          id="ruleset-geo-source-url"
          onChange={(event) => {
            setSaved(false);
            setForm((current) => ({ ...current, geoSourceUrl: event.currentTarget.value }));
          }}
          value={form.geoSourceUrl}
        />
      </div>

      <div className="grid gap-1.5">
        <Label className="text-muted-foreground" htmlFor="ruleset-srs-source-url">
          {t("options.srsSource")}
        </Label>
        <Input
          id="ruleset-srs-source-url"
          onChange={(event) => {
            setSaved(false);
            setForm((current) => ({ ...current, srsSourceUrl: event.currentTarget.value }));
          }}
          value={form.srsSourceUrl}
        />
      </div>

      <div className="flex items-center gap-2">
        <Button disabled={working} onClick={() => void save()} type="button" variant="outline">
          <Save className="size-4" aria-hidden="true" />
          {t("actions.save")}
        </Button>
        {saved ? <span className="text-xs text-muted-foreground">{t("options.saved")}</span> : null}
        {error ? <span className="text-xs text-destructive">{error}</span> : null}
      </div>
    </section>
  );
}
