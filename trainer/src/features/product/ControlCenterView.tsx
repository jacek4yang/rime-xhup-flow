/**
 * 控制中心:Rime 环境检测、安装/升级/修复/卸载、学习数据管理与诊断。
 *
 * 全部业务在桌面应用的 Rust 侧(`trainer/src-tauri`);本组件只展示
 * 状态与计划、转发确认后的操作。浏览器环境显示「需要桌面应用」提示。
 */

import { useCallback, useEffect, useState } from "react";
import { ClipboardCheck, Download, FileText, RefreshCw, Stethoscope, Trash2, Upload } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogTitle,
} from "@/components/ui/dialog";
import { Separator } from "@/components/ui/separator";
import type { I18nKey } from "@/lib/i18n";
import { useI18n } from "@/lib/use-i18n";
import {
  isDesktopApp,
  productApi,
  type MaintenanceKind,
  type PlanActionDto,
  type PlanDto,
  type ProductStatusDto,
  type RimeClient,
} from "@/lib/product";

const CLIENT_LABELS: Record<RimeClient, I18nKey> = {
  Weasel: "product.clientWeasel",
  Squirrel: "product.clientSquirrel",
  Fcitx5: "product.clientFcitx5",
  Ibus: "product.clientIbus",
};

const PLAN_ACTION_LABELS: Record<PlanActionDto["kind"], I18nKey> = {
  write: "product.planWrite",
  overwrite: "product.planOverwrite",
  delete: "product.planDelete",
};

function errorReason(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}

/** 依据当前安装状态决定主维护按钮文案与类型。 */
function mainAction(status: ProductStatusDto): { kind: MaintenanceKind; label: I18nKey } {
  const install = status.install;
  if (!install || install.installed_files === 0) {
    return { kind: "install", label: "product.actionInstall" };
  }
  if (install.missing_files.length > 0) {
    return { kind: "install", label: "product.actionRepair" };
  }
  if (status.update_available) {
    return { kind: "install", label: "product.actionUpdate" };
  }
  return { kind: "install", label: "product.actionRepair" };
}

export function ControlCenterView() {
  const { t } = useI18n();
  const desktop = isDesktopApp();
  const [status, setStatus] = useState<ProductStatusDto | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [plan, setPlan] = useState<{ kind: MaintenanceKind; plan: PlanDto } | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [uninstallOpen, setUninstallOpen] = useState(false);
  const [resetOpen, setResetOpen] = useState(false);
  const [importPath, setImportPath] = useState("");
  const [diagnostics, setDiagnostics] = useState<string | null>(null);

  const refresh = useCallback(() => {
    productApi
      .status()
      .then((next) => {
        setStatus(next);
        setLoadError(null);
      })
      .catch((cause: unknown) => setLoadError(errorReason(cause)));
  }, []);

  useEffect(refresh, [refresh]);

  const run = useCallback(
    async (action: () => Promise<string | null>) => {
      setBusy(true);
      setError(null);
      setNotice(null);
      try {
        const message = await action();
        if (message !== null) setNotice(message);
        refresh();
      } catch (cause: unknown) {
        setError(t("product.failed", { reason: errorReason(cause) }));
      } finally {
        setBusy(false);
      }
    },
    [refresh, t],
  );

  const openPlan = (kind: MaintenanceKind) => {
    setNotice(null);
    setError(null);
    productApi
      .plan(kind)
      .then((next) => setPlan({ kind, plan: next }))
      .catch((cause: unknown) => setError(t("product.failed", { reason: errorReason(cause) })));
  };

  const confirmPlan = () => {
    if (!plan) return;
    const { kind } = plan;
    setPlan(null);
    void run(async () => {
      const result = await productApi.execute(kind);
      return `${t("product.executeDone", { n: result.done })} ${t("product.redeployRequired", { guidance: result.redeploy_guidance })}`;
    });
  };

  if (!desktop) {
    return (
      <section aria-labelledby="product-title" className="flex flex-col gap-4">
        <header>
          <h1 id="product-title" className="text-xl font-semibold">
            {t("product.title")}
          </h1>
          <p className="mt-1 text-sm text-muted-foreground">{t("product.subtitle")}</p>
        </header>
        <Card>
          <CardContent className="pt-6">
            <p className="text-sm text-muted-foreground">{t("product.notDesktop")}</p>
          </CardContent>
        </Card>
      </section>
    );
  }

  const main = status ? mainAction(status) : null;
  const install = status?.install ?? null;
  const learning = status?.learning ?? null;

  return (
    <section aria-labelledby="product-title" className="flex flex-col gap-4">
      <header className="flex flex-wrap items-center justify-between gap-2">
        <div>
          <h1 id="product-title" className="text-xl font-semibold">
            {t("product.title")}
          </h1>
          <p className="mt-1 text-sm text-muted-foreground">{t("product.subtitle")}</p>
        </div>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={refresh}
          disabled={busy}
          aria-label={t("product.refresh")}
          title={t("product.refresh")}
        >
          <RefreshCw className="size-4" aria-hidden />
        </Button>
      </header>

      {loadError && (
        <Card>
          <CardContent className="pt-6">
            <p className="text-sm text-destructive">
              {t("product.failed", { reason: loadError })}
            </p>
          </CardContent>
        </Card>
      )}
      {error && (
        <p role="alert" className="text-sm text-destructive">
          {error}
        </p>
      )}
      {notice && <p className="text-sm text-primary">{notice}</p>}

      {status && (
        <>
          <Card>
            <CardHeader>
              <CardTitle>{t("product.installState")}</CardTitle>
              <CardDescription>
                {t(CLIENT_LABELS[status.client])}
                {" · "}
                {status.rime_detected ? t("product.detected") : t("product.notDetected")}
              </CardDescription>
            </CardHeader>
            <CardContent className="flex flex-col gap-3 text-sm">
              {!status.rime_detected ? (
                <p className="text-muted-foreground">{t("product.notDetectedHint")}</p>
              ) : (
                <>
                  <Row label={t("product.userDataDir")}>
                    <code className="text-xs">{status.user_data_dir}</code>
                  </Row>
                  <Row label={t("product.installState")}>
                    {install && install.installed_files > 0 ? (
                      <span className="flex flex-wrap items-center gap-2">
                        <Badge variant={install.missing_files.length > 0 ? "destructive" : "secondary"}>
                          {t("product.installed", {
                            done: install.installed_files,
                            total: install.total_files,
                          })}
                        </Badge>
                        {install.missing_files.length > 0
                          ? t("product.broken", { n: install.missing_files.length })
                          : null}
                      </span>
                    ) : (
                      <Badge variant="outline">{t("product.notInstalled")}</Badge>
                    )}
                  </Row>
                  {install?.installed_version && (
                    <Row label={t("product.installedVersion")}>
                      <span className="flex items-center gap-2">
                        {install.installed_version}
                        <Badge variant={status.update_available ? "destructive" : "secondary"}>
                          {status.update_available
                            ? t("product.updateAvailable")
                            : t("product.upToDate")}
                        </Badge>
                      </span>
                    </Row>
                  )}
                  <Row label={t("product.bundledVersion")}>{status.bundled_version}</Row>
                  {install && install.schemas.length > 0 && (
                    <Row label={t("product.schemaMode")}>
                      <code className="text-xs">{install.schemas.join(" · ")}</code>
                    </Row>
                  )}
                  <p className="text-xs text-muted-foreground">{t("product.schemaHint")}</p>
                </>
              )}
            </CardContent>
          </Card>

          {status.rime_detected && (
            <Card>
              <CardHeader>
                <CardTitle>{t("product.maintenance")}</CardTitle>
                <CardDescription>{t("product.backupHint")}</CardDescription>
              </CardHeader>
              <CardContent className="flex flex-wrap gap-2">
                {main && (
                  <Button type="button" onClick={() => openPlan("install")} disabled={busy}>
                    {t(main.label)}
                  </Button>
                )}
                <Button
                  type="button"
                  variant="destructive"
                  onClick={() => {
                    setNotice(null);
                    setError(null);
                    if (!install || install.installed_files === 0) {
                      setNotice(t("product.uninstallNothing"));
                      return;
                    }
                    setUninstallOpen(true);
                  }}
                  disabled={busy}
                >
                  {t("product.actionUninstall")}
                </Button>
              </CardContent>
            </Card>
          )}

          {status.rime_detected && learning && (
            <Card>
              <CardHeader>
                <CardTitle>{t("product.learning")}</CardTitle>
                <CardDescription>{t("product.learningPrivacy")}</CardDescription>
              </CardHeader>
              <CardContent className="flex flex-col gap-3 text-sm">
                <p>{learning.db_exists ? t("product.learningDbExists") : t("product.learningDbEmpty")}</p>
                {learning.snapshot_available && (
                  <p className="text-muted-foreground">{t("product.learningSnapshot")}</p>
                )}
                {!learning.tool_available && (
                  <p className="text-muted-foreground">{t("product.learningToolMissing")}</p>
                )}
                <div className="flex flex-wrap items-center gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    onClick={() =>
                      void run(async () =>
                        t("product.learningExported", { path: await productApi.learningExport() }),
                      )
                    }
                    disabled={busy || !learning.tool_available}
                  >
                    <Download className="size-4" aria-hidden />
                    {t("product.learningExport")}
                  </Button>
                </div>
                <Separator />
                <div className="flex flex-col gap-2">
                  <label htmlFor="learning-import-path" className="text-sm font-medium">
                    {t("product.learningImportPath")}
                  </label>
                  <div className="flex flex-wrap gap-2">
                    <input
                      id="learning-import-path"
                      type="text"
                      value={importPath}
                      onChange={(event) => setImportPath(event.target.value)}
                      className="min-h-11 flex-1 rounded-md border border-input bg-background px-3 py-2 text-sm"
                    />
                    <Button
                      type="button"
                      variant="outline"
                      onClick={() =>
                        void run(async () => {
                          await productApi.learningImport(importPath.trim());
                          return t("product.learningImported");
                        })
                      }
                      disabled={busy || importPath.trim() === ""}
                    >
                      <Upload className="size-4" aria-hidden />
                      {t("product.learningImport")}
                    </Button>
                  </div>
                </div>
                <Button
                  type="button"
                  variant="destructive"
                  className="self-start"
                  onClick={() => setResetOpen(true)}
                  disabled={busy}
                >
                  <Trash2 className="size-4" aria-hidden />
                  {t("product.learningReset")}
                </Button>
              </CardContent>
            </Card>
          )}

          {status.rime_detected && (
            <Card>
              <CardHeader>
                <CardTitle>{t("product.diagnostics")}</CardTitle>
                <CardDescription>{t("product.diagnosticsHint")}</CardDescription>
              </CardHeader>
              <CardContent className="flex flex-col gap-3">
                <div className="flex flex-wrap gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    onClick={() =>
                      void productApi
                        .diagnostics()
                        .then(setDiagnostics)
                        .catch((cause: unknown) =>
                          setError(t("product.failed", { reason: errorReason(cause) })),
                        )
                    }
                    disabled={busy}
                  >
                    <Stethoscope className="size-4" aria-hidden />
                    {t("product.diagnosticsGenerate")}
                  </Button>
                  {diagnostics && (
                    <Button
                      type="button"
                      variant="outline"
                      onClick={() => {
                        void navigator.clipboard
                          ?.writeText(diagnostics)
                          .then(() => setNotice(t("product.diagnosticsCopied")))
                          .catch(() => setNotice(t("product.diagnosticsCopied")));
                      }}
                    >
                      <ClipboardCheck className="size-4" aria-hidden />
                      {t("product.diagnosticsCopied")}
                    </Button>
                  )}
                </div>
                {diagnostics && (
                  <pre
                    className="max-h-64 overflow-auto rounded-md bg-muted p-3 text-xs"
                    aria-label={t("product.diagnostics")}
                  >
                    {diagnostics}
                  </pre>
                )}
              </CardContent>
            </Card>
          )}
        </>
      )}

      {/* 维护计划确认:先看清楚每一个动作,再执行。 */}
      <Dialog open={plan !== null} onOpenChange={(open) => !open && setPlan(null)}>
        <DialogContent aria-labelledby="plan-title">
          <DialogTitle id="plan-title">{t("product.planTitle")}</DialogTitle>
          {plan && (
            <>
              <DialogDescription>
                <FileText className="mr-1 inline size-4" aria-hidden />
                {plan.plan.actions.length}
              </DialogDescription>
              <ul className="max-h-64 overflow-auto text-sm">
                {plan.plan.actions.map((action) => (
                  <li key={`${action.kind}:${action.file}`} className="flex items-baseline gap-2 py-0.5">
                    <Badge variant="outline" className="shrink-0">
                      {t(PLAN_ACTION_LABELS[action.kind])}
                    </Badge>
                    <code className="text-xs">{action.file}</code>
                  </li>
                ))}
              </ul>
              <DialogFooter>
                <Button type="button" variant="outline" onClick={() => setPlan(null)}>
                  {t("common.cancel")}
                </Button>
                <Button type="button" onClick={confirmPlan} disabled={busy}>
                  {t("product.planConfirm")}
                </Button>
              </DialogFooter>
            </>
          )}
        </DialogContent>
      </Dialog>

      {/* 卸载确认:只删 XHUP 拥有文件,但仍是破坏性操作。 */}
      <Dialog open={uninstallOpen} onOpenChange={setUninstallOpen}>
        <DialogContent aria-labelledby="uninstall-title">
          <DialogTitle id="uninstall-title">{t("product.uninstallConfirmTitle")}</DialogTitle>
          <DialogDescription>
            {t("product.uninstallConfirmBody", {
              n: install?.installed_files ?? 0,
            })}
          </DialogDescription>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setUninstallOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button
              type="button"
              variant="destructive"
              onClick={() => {
                setUninstallOpen(false);
                void run(async () => {
                  const result = await productApi.execute("uninstall");
                  return `${t("product.executeDone", { n: result.done })} ${t("product.redeployRequired", { guidance: result.redeploy_guidance })}`;
                });
              }}
              disabled={busy}
            >
              {t("product.actionUninstall")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* 学习重置确认:破坏性,独立于卸载。 */}
      <Dialog open={resetOpen} onOpenChange={setResetOpen}>
        <DialogContent aria-labelledby="reset-title">
          <DialogTitle id="reset-title">{t("product.learningResetConfirmTitle")}</DialogTitle>
          <DialogDescription>{t("product.learningResetConfirmBody")}</DialogDescription>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setResetOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button
              type="button"
              variant="destructive"
              onClick={() => {
                setResetOpen(false);
                void run(async () => {
                  await productApi.learningReset(true);
                  return t("product.learningImported");
                });
              }}
              disabled={busy}
            >
              {t("product.learningReset")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </section>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-2">
      <span className="text-muted-foreground">{label}</span>
      <span className="text-right">{children}</span>
    </div>
  );
}
