import { useEffect, useId, useMemo, useState } from "react";
import type * as React from "react";
import { AlertTriangle, CheckCircle2, Database, Download, RefreshCw, Upload } from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useI18n } from "@/i18n/use-i18n";
import { cn } from "@/lib/utils";
import {
  backupCreateLocal,
  backupRestoreLocal,
  backupSaveWebdavSettings,
  backupStatus,
  backupWebdavCheck,
  backupWebdavPull,
  backupWebdavPush,
} from "@/ipc";
import type { BackupOperationResult, BackupRemoteResult, BackupStatus_Serialize, WebDavItem_Deserialize } from "@/ipc/bindings";

type WorkingAction = "localBackup" | "localRestore" | "save" | "webdavCheck" | "webdavPull" | "webdavPush";

const emptyWebDav: WebDavItem_Deserialize = {
  DirName: null,
  Password: null,
  Url: null,
  UserName: null,
};

export function BackupDialog() {
  const queryClient = useQueryClient();
  const { t } = useI18n();
  const [error, setError] = useState<string | null>(null);
  const [localOutputPath, setLocalOutputPath] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [restorePath, setRestorePath] = useState("");
  const [status, setStatus] = useState<BackupStatus_Serialize | null>(null);
  const [webDav, setWebDav] = useState<WebDavItem_Deserialize>(emptyWebDav);
  const [working, setWorking] = useState<WorkingAction | null>(null);

  useEffect(() => {
    let disposed = false;

    void backupStatus()
      .then((nextStatus) => {
        if (disposed) {
          return;
        }
        setStatus(nextStatus);
        setLocalOutputPath(nextStatus.defaultBackupPath);
        setWebDav(nextStatus.webDavItem);
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

  const normalizedWebDav = useMemo(() => normalizeWebDav(webDav), [webDav]);

  async function run(action: WorkingAction) {
    setWorking(action);
    setError(null);
    setMessage(null);
    try {
      if (action === "save") {
        await backupSaveWebdavSettings(normalizedWebDav);
        setMessage(t("backup.saved"));
        return;
      }
      if (action === "localBackup") {
        const result = await backupCreateLocal(normalizeText(localOutputPath));
        setMessage(operationMessage(result));
        return;
      }
      if (action === "localRestore") {
        const inputPath = normalizeText(restorePath);
        if (!inputPath) {
          setError(t("backup.restorePathRequired"));
          return;
        }
        const result = await backupRestoreLocal(inputPath);
        await invalidateRestoredState(queryClient);
        setMessage(result.message);
        return;
      }
      if (action === "webdavCheck") {
        const result = await backupWebdavCheck(normalizedWebDav);
        setMessage(result.message);
        return;
      }
      if (action === "webdavPush") {
        const result = await backupWebdavPush(normalizedWebDav);
        setMessage(remoteMessage(result));
        return;
      }
      if (action === "webdavPull") {
        const result = await backupWebdavPull(normalizedWebDav);
        await invalidateRestoredState(queryClient);
        setMessage(result.message);
      }
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
    } finally {
      setWorking(null);
    }
  }

  function updateWebDav(key: keyof WebDavItem_Deserialize, value: string) {
    setWebDav((current) => ({ ...current, [key]: value }));
  }

  return (
    <DialogContent className="max-w-3xl">
      <DialogHeader>
        <DialogTitle className="flex items-center gap-2">
          <Database className="size-4" aria-hidden="true" />
          {t("backup.title")}
        </DialogTitle>
        <DialogDescription className="sr-only">{t("backup.description")}</DialogDescription>
      </DialogHeader>

      <div className="grid gap-5">
        {message ? (
          <Alert
            className="border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300"
            role="status"
          >
            <CheckCircle2 aria-hidden="true" />
            <AlertDescription className="text-current">{message}</AlertDescription>
          </Alert>
        ) : null}
        {error ? (
          <Alert variant="destructive">
            <AlertTriangle aria-hidden="true" />
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : null}

        <Card className="gap-3 rounded-md bg-background p-3 shadow-none">
          <CardHeader className="p-0">
            <CardTitle className="flex items-center gap-2 text-sm">
              <Database className="size-4 text-muted-foreground" aria-hidden="true" />
              {t("backup.local")}
            </CardTitle>
          </CardHeader>
          <CardContent className="grid gap-3 p-0">
            <div className="grid gap-3 sm:grid-cols-[1fr_auto]">
              <TextField
                label={t("backup.outputPath")}
                onChange={setLocalOutputPath}
                value={localOutputPath}
              />
              <Button
                className="self-end"
                disabled={working !== null}
                onClick={() => void run("localBackup")}
                type="button"
              >
                <Database className="size-4" aria-hidden="true" />
                {working === "localBackup" ? t("backup.working") : t("backup.createLocal")}
              </Button>
            </div>
            <div className="grid gap-3 sm:grid-cols-[1fr_auto]">
              <TextField
                label={t("backup.restorePath")}
                onChange={setRestorePath}
                value={restorePath}
              />
              <Button
                className="self-end"
                disabled={working !== null || !restorePath.trim()}
                onClick={() => void run("localRestore")}
                type="button"
                variant="outline"
              >
                <Download className="size-4" aria-hidden="true" />
                {working === "localRestore" ? t("backup.working") : t("backup.restoreLocal")}
              </Button>
            </div>
            {status?.backupDir ? (
              <Badge className="max-w-full justify-start truncate" title={status.backupDir} variant="outline">
                {status.backupDir}
              </Badge>
            ) : null}
          </CardContent>
        </Card>

        <Card className="gap-3 rounded-md bg-background p-3 shadow-none">
          <CardHeader className="p-0">
            <CardTitle className="flex items-center gap-2 text-sm">
              <Upload className="size-4 text-muted-foreground" aria-hidden="true" />
              {t("backup.webdav")}
            </CardTitle>
          </CardHeader>
          <CardContent className="grid gap-3 p-0">
            <div className="grid gap-3 sm:grid-cols-2">
              <TextField label={t("backup.webdavUrl")} onChange={(value) => updateWebDav("Url", value)} value={webDav.Url} />
              <TextField
                label={t("backup.webdavDir")}
                onChange={(value) => updateWebDav("DirName", value)}
                value={webDav.DirName}
              />
              <TextField
                label={t("backup.webdavUser")}
                onChange={(value) => updateWebDav("UserName", value)}
                value={webDav.UserName}
              />
              <TextField
                label={t("backup.webdavPassword")}
                onChange={(value) => updateWebDav("Password", value)}
                type="password"
                value={webDav.Password}
              />
            </div>

            <div className="flex flex-wrap items-center gap-2">
              <Button disabled={working !== null} onClick={() => void run("save")} type="button" variant="outline">
                {working === "save" ? t("backup.working") : t("actions.save")}
              </Button>
              <Button disabled={working !== null} onClick={() => void run("webdavCheck")} type="button" variant="outline">
                <RefreshCw className="size-4" aria-hidden="true" />
                {working === "webdavCheck" ? t("backup.working") : t("backup.webdavCheck")}
              </Button>
              <Button disabled={working !== null} onClick={() => void run("webdavPush")} type="button">
                <Upload className="size-4" aria-hidden="true" />
                {working === "webdavPush" ? t("backup.working") : t("backup.webdavPush")}
              </Button>
              <Button disabled={working !== null} onClick={() => void run("webdavPull")} type="button" variant="secondary">
                <Download className="size-4" aria-hidden="true" />
                {working === "webdavPull" ? t("backup.working") : t("backup.webdavPull")}
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>

      <DialogFooter />
    </DialogContent>
  );
}

function TextField({
  className,
  id,
  label,
  onChange,
  type = "text",
  value,
  ...props
}: Omit<React.ComponentProps<typeof Input>, "onChange" | "value"> & {
  label: string;
  onChange: (value: string) => void;
  type?: "password" | "text";
  value?: string | null;
}) {
  const generatedId = useId();
  const inputId = id ?? generatedId;

  return (
    <div className="grid min-w-0 gap-1">
      <Label className="text-xs text-muted-foreground" htmlFor={inputId}>
        <span className="truncate">{label}</span>
      </Label>
      <Input
        className={cn("bg-card", className)}
        id={inputId}
        onChange={(event) => onChange(event.target.value)}
        type={type}
        value={value ?? ""}
        {...props}
      />
    </div>
  );
}

function normalizeWebDav(settings: WebDavItem_Deserialize): WebDavItem_Deserialize {
  return {
    DirName: normalizeText(settings.DirName),
    Password: settings.Password || null,
    Url: normalizeText(settings.Url),
    UserName: normalizeText(settings.UserName),
  };
}

function normalizeText(value: string | null | undefined) {
  const trimmed = value?.trim() ?? "";

  return trimmed.length > 0 ? trimmed : null;
}

function operationMessage(result: BackupOperationResult) {
  const details = result.path ? `: ${result.path}` : "";
  const size = result.bytes ? ` (${formatBytes(result.bytes)})` : "";

  return `${result.message}${details}${size}`;
}

function remoteMessage(result: BackupRemoteResult) {
  const size = result.bytes ? ` (${formatBytes(result.bytes)})` : "";

  return `${result.message}: ${result.remotePath}${size}`;
}

function formatBytes(bytes: number) {
  if (bytes < 1024) {
    return `${bytes.toFixed(0)} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KiB`;
  }

  return `${(bytes / 1024 / 1024).toFixed(1)} MiB`;
}

async function invalidateRestoredState(queryClient: ReturnType<typeof useQueryClient>) {
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: ["app-config"] }),
    queryClient.invalidateQueries({ queryKey: ["backup"] }),
    queryClient.invalidateQueries({ queryKey: ["profiles"] }),
    queryClient.invalidateQueries({ queryKey: ["profile-ex"] }),
    queryClient.invalidateQueries({ queryKey: ["subscriptions"] }),
    queryClient.invalidateQueries({ queryKey: ["routings"] }),
    queryClient.invalidateQueries({ queryKey: ["dns"] }),
  ]);
}
