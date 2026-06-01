import { useEffect, useMemo, useState } from "react";
import { AlertTriangle, CheckCircle2, Database, Download, RefreshCw, Upload } from "lucide-react";
import { useQueryClient } from "@tanstack/react-query";

import { Button } from "@/components/ui/button";
import {
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useI18n } from "@/i18n/use-i18n";
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
          <div className="flex items-start gap-2 rounded-md border border-emerald-500/30 bg-emerald-500/10 px-3 py-2 text-sm text-emerald-700 dark:text-emerald-300">
            <CheckCircle2 className="mt-0.5 size-4 shrink-0" aria-hidden="true" />
            <span>{message}</span>
          </div>
        ) : null}
        {error ? (
          <div className="flex items-start gap-2 rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">
            <AlertTriangle className="mt-0.5 size-4 shrink-0" aria-hidden="true" />
            <span>{error}</span>
          </div>
        ) : null}

        <section className="grid gap-3">
          <div className="flex items-center gap-2">
            <Database className="size-4 text-muted-foreground" aria-hidden="true" />
            <h3 className="text-sm font-medium">{t("backup.local")}</h3>
          </div>
          <div className="grid gap-3 sm:grid-cols-[1fr_auto]">
            <label className="grid gap-1 text-sm">
              <span className="font-medium">{t("backup.outputPath")}</span>
              <input
                className="h-9 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
                onChange={(event) => setLocalOutputPath(event.target.value)}
                value={localOutputPath}
              />
            </label>
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
            <label className="grid gap-1 text-sm">
              <span className="font-medium">{t("backup.restorePath")}</span>
              <input
                className="h-9 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
                onChange={(event) => setRestorePath(event.target.value)}
                value={restorePath}
              />
            </label>
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
          {status?.backupDir ? <p className="text-xs text-muted-foreground">{status.backupDir}</p> : null}
        </section>

        <section className="grid gap-3 border-t pt-4">
          <div className="flex items-center gap-2">
            <Upload className="size-4 text-muted-foreground" aria-hidden="true" />
            <h3 className="text-sm font-medium">{t("backup.webdav")}</h3>
          </div>

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
        </section>
      </div>

      <DialogFooter />
    </DialogContent>
  );
}

function TextField({
  label,
  onChange,
  type = "text",
  value,
}: {
  label: string;
  onChange: (value: string) => void;
  type?: "password" | "text";
  value?: string | null;
}) {
  return (
    <label className="grid gap-1 text-sm">
      <span className="font-medium">{label}</span>
      <input
        className="h-9 rounded-md border bg-background px-3 text-sm outline-none ring-offset-background focus-visible:ring-2 focus-visible:ring-ring"
        onChange={(event) => onChange(event.target.value)}
        type={type}
        value={value ?? ""}
      />
    </label>
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
