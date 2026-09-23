import { useState } from "react";
import { RotateCcw } from "lucide-react";

import { useI18n } from "@voya/i18n/use-i18n";
import { Button } from "@voya/ui/components/button";
import { ConfirmDialog } from "@voya/ui/components/confirm-dialog";
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  ScrollableDialogContent,
} from "@voya/ui/components/dialog";
import { Disclosure } from "@voya/ui/components/disclosure";

import {
  NumberField,
  SettingsFields,
  SettingsSwitch,
  TextField,
} from "@/features/settings/settings-form";
import type { SelfHostConfig, SelfHostState } from "@voya/contracts";

import type { SelfHostController } from "./use-self-host";

const MAX_PORT = 65535;
/** The fields behind "Advanced" whose rejection has to open it. */
const ADVANCED_FIELDS = ["vlessPort", "shadowsocksPort", "realityServerName", "realityServerPort"];
const ROWS_CLASS = "grid divide-y divide-border-subtle [&>*]:py-3 [&>*:first-child]:pt-0";

/** `host` or `host:port`; the port is the last colon followed by digits only. */
const SITE_PATTERN = /^(.*?)(?::(\d+))?$/;

function parseSite(text: string) {
  const match = SITE_PATTERN.exec(text.trim());
  return { host: match?.[1] ?? "", port: match?.[2] === undefined ? null : Number(match[2]) };
}

/**
 * The node's settings. An empty field means "use the default", which its
 * placeholder names, and every change saves at once; a change that alters the
 * server (a port, a protocol, the disguise site) restarts it, which drops the
 * connections of devices using it for a moment.
 */
export function NodeSettingsDialog({
  controller,
  onOpenChange,
  state,
}: {
  controller: SelfHostController;
  onOpenChange: (open: boolean) => void;
  state: SelfHostState;
}) {
  const { t } = useI18n();
  const [confirmReset, setConfirmReset] = useState(false);
  const { config, defaults } = state;
  const save = controller.saveConfig;

  // The disguise site and its port share one field, so they share its error.
  const siteError = controller.fieldErrors.realityServerName ?? controller.fieldErrors.realityServerPort;
  const errors = siteError
    ? { ...controller.fieldErrors, realityServerName: siteError }
    : controller.fieldErrors;
  const advancedInvalid =
    state.runtime.problem === "portInUse" || ADVANCED_FIELDS.some((field) => field in errors);

  const siteIsDefault =
    config.realityServerName === defaults.realityServerName &&
    config.realityServerPort === defaults.realityServerPort;
  const site = siteIsDefault
    ? ""
    : config.realityServerPort === defaults.realityServerPort
      ? config.realityServerName
      : `${config.realityServerName}:${config.realityServerPort}`;

  const isDefault = (Object.keys(defaults) as (keyof SelfHostConfig)[]).every(
    (key) => key === "enabled" || config[key] === defaults[key],
  );

  return (
    <Dialog open onOpenChange={onOpenChange}>
      <ScrollableDialogContent closeLabel={t("actions.close")} width="40rem">
        <DialogHeader>
          <DialogTitle>{t("panes.selfHost.config.title")}</DialogTitle>
          <DialogDescription>{t("panes.selfHost.config.description")}</DialogDescription>
        </DialogHeader>
        <DialogBody className="@container grid gap-4">
          <SettingsFields errors={errors}>
            <div className={ROWS_CLASS}>
              <TextField
                description={t("panes.selfHost.config.deviceLabelHint")}
                field="deviceLabel"
                id="self-host-device-label"
                label={t("panes.selfHost.config.deviceLabel")}
                onChange={(deviceLabel) => void save({ deviceLabel })}
                placeholder="VoyaVPN"
                value={config.deviceLabel}
              />
              <SettingsSwitch
                checked={config.vlessEnabled}
                description={t("panes.selfHost.config.vlessHint")}
                field="protocols"
                label={t("panes.selfHost.config.vless")}
                onCheckedChange={(vlessEnabled) => void save({ vlessEnabled })}
              />
              <SettingsSwitch
                checked={config.shadowsocksEnabled}
                description={t("panes.selfHost.config.shadowsocksHint")}
                label={t("panes.selfHost.config.shadowsocks")}
                onCheckedChange={(shadowsocksEnabled) => void save({ shadowsocksEnabled })}
              />
              <TextField
                description={t("panes.selfHost.config.customAddressHint")}
                field="customAddress"
                id="self-host-custom-address"
                label={t("panes.selfHost.config.customAddress")}
                onChange={(address) => void save({ customAddress: address.trim() || null })}
                placeholder={t("panes.selfHost.config.customAddressAuto")}
                value={config.customAddress ?? ""}
              />
            </div>
            <Disclosure invalid={advancedInvalid} title={t("panes.selfHost.config.advanced")}>
              <div className={ROWS_CLASS}>
                {config.vlessEnabled ? (
                  <>
                    <NumberField
                      description={t("panes.selfHost.config.portHint")}
                      field="vlessPort"
                      id="self-host-vless-port"
                      label={t("panes.selfHost.config.vlessPort")}
                      nullable
                      onChange={(port) => void save({ vlessPort: port ?? defaults.vlessPort })}
                      placeholder={t("panes.selfHost.config.portAuto")}
                      value={config.vlessPort === defaults.vlessPort ? null : config.vlessPort}
                    />
                    <TextField
                      description={t("panes.selfHost.config.realityServerHint")}
                      field="realityServerName"
                      id="self-host-reality-server"
                      label={t("panes.selfHost.config.realityServer")}
                      onChange={(text) => {
                        const { host, port } = parseSite(text);
                        void save({
                          realityServerName: host || defaults.realityServerName,
                          realityServerPort: port ?? defaults.realityServerPort,
                        });
                      }}
                      placeholder={defaults.realityServerName}
                      validate={(text) => {
                        const { port } = parseSite(text);
                        return port === null || (port >= 1 && port <= MAX_PORT)
                          ? undefined
                          : t("validation.invalid");
                      }}
                      value={site}
                    />
                  </>
                ) : null}
                {config.shadowsocksEnabled ? (
                  <NumberField
                    description={t("panes.selfHost.config.portHint")}
                    field="shadowsocksPort"
                    id="self-host-shadowsocks-port"
                    label={t("panes.selfHost.config.shadowsocksPort")}
                    nullable
                    onChange={(port) => void save({ shadowsocksPort: port ?? defaults.shadowsocksPort })}
                    placeholder={t("panes.selfHost.config.portAuto")}
                    value={config.shadowsocksPort === defaults.shadowsocksPort ? null : config.shadowsocksPort}
                  />
                ) : null}
                <SettingsSwitch
                  checked={config.upnpEnabled}
                  description={t("panes.selfHost.config.upnpHint")}
                  label={t("panes.selfHost.config.upnp")}
                  onCheckedChange={(upnpEnabled) => void save({ upnpEnabled })}
                />
                <SettingsSwitch
                  checked={config.allowLanAccess}
                  description={t("panes.selfHost.config.allowLanHint")}
                  label={t("panes.selfHost.config.allowLan")}
                  onCheckedChange={(allowLanAccess) => void save({ allowLanAccess })}
                />
                <SettingsSwitch
                  checked={config.blockBittorrent}
                  description={t("panes.selfHost.config.blockBittorrentHint")}
                  label={t("panes.selfHost.config.blockBittorrent")}
                  onCheckedChange={(blockBittorrent) => void save({ blockBittorrent })}
                />
              </div>
            </Disclosure>
          </SettingsFields>
        </DialogBody>
        <DialogFooter className="items-center sm:justify-between">
          <Button
            disabled={isDefault || controller.pending === "save"}
            onClick={() => setConfirmReset(true)}
            type="button"
            variant="outline"
          >
            <RotateCcw aria-hidden="true" className="size-4" />
            {t("panes.selfHost.config.reset")}
          </Button>
          <Button onClick={() => onOpenChange(false)} type="button">
            {t("actions.done")}
          </Button>
        </DialogFooter>
        <ConfirmDialog
          cancelLabel={t("actions.cancel")}
          confirmLabel={t("panes.selfHost.config.reset")}
          description={t("panes.selfHost.config.resetDescription")}
          destructive
          onConfirm={() => {
            setConfirmReset(false);
            // Hosting is the page's switch, not a setting: a reset leaves it alone.
            void save({ ...defaults, enabled: config.enabled });
          }}
          onOpenChange={setConfirmReset}
          open={confirmReset}
          title={t("panes.selfHost.config.resetTitle")}
        />
      </ScrollableDialogContent>
    </Dialog>
  );
}
