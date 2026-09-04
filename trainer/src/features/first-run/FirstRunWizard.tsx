/**
 * 首次启动向导:欢迎 → 检测 → 方案说明 → 安装计划 → 部署 → 验证 →
 * 试打 → 新手训练。
 *
 * 全部业务判断在 `state-machine.ts`(纯状态机)与 Rust 侧管理器;
 * 本组件只做三件事:渲染当前步骤、把用户操作翻译为事件、把
 * productApi 的异步结果回灌状态机。已有完成/跳过记录时不渲染。
 */

import { useEffect, useReducer, useRef, useState } from "react";
import { CheckCircle2, ClipboardCopy, Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogTitle,
} from "@/components/ui/dialog";
import { CommandError } from "@/lib/native";
import type { I18nKey } from "@/lib/i18n";
import { useI18n } from "@/lib/use-i18n";
import {
  ERROR_CODES,
  productApi,
  type PlanActionDto,
  type ProductStatusDto,
  type RimeClient,
} from "@/lib/product";
import type { PracticeMode } from "@/features/practice/types";
import { readOnboarding, writeOnboarding } from "./onboarding";
import {
  initialFirstRunState,
  isTerminal,
  STARTER_TRAINING_PATH,
  transition,
  type FirstRunEvent,
  type FirstRunState,
  type SchemaChoice,
} from "./state-machine";

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

type Translate = (key: I18nKey, params?: Record<string, string | number>) => string;

/** 错误呈现:优先稳定错误码,未知退回人读消息(与控制中心同策略)。 */
function errorText(error: unknown, t: Translate): string {
  if (error instanceof CommandError) {
    if ((ERROR_CODES as readonly string[]).includes(error.code)) {
      return t(`errorCodes.${error.code}` as I18nKey);
    }
    return error.message;
  }
  if (error instanceof Error) return error.message;
  return String(error);
}

export interface FirstRunWizardProps {
  /** 终局选择「开始训练」时回调(跳入练习视图的推荐首步)。 */
  onStartTraining: (mode: PracticeMode) => void;
  /** 用户要求直接进入控制中心(已健康/需要人工处理时)。 */
  onOpenControlCenter: () => void;
  /** 重新运行信号:每次自增触发向导重新评估(设置页入口)。 */
  reopenSignal?: number;
}

export function FirstRunWizard({
  onStartTraining,
  onOpenControlCenter,
  reopenSignal = 0,
}: FirstRunWizardProps) {
  const { t } = useI18n();
  const [state, dispatch] = useReducer(transition, undefined, initialFirstRunState);
  const [diagnosticsCopied, setDiagnosticsCopied] = useState(false);

  // 已完成/跳过过引导:静默不渲染(高级用户不被向导困住)。
  const [record, setRecord] = useState(readOnboarding);
  const [dismissed, setDismissed] = useState(false);
  const outcomeHandled = useRef(false);

  // 设置页「重新运行引导」:记录已被清除,重置终局标记并重新打开。
  const lastSignal = useRef(reopenSignal);
  useEffect(() => {
    if (reopenSignal === lastSignal.current) return;
    lastSignal.current = reopenSignal;
    outcomeHandled.current = false;
    setDismissed(false);
    setRecord(readOnboarding());
  }, [reopenSignal]);

  // 异步驱动:进入异步步骤时发起对应命令,结果/错误回灌状态机。
  // 以 state 对象为依赖:同一步骤的重试(新对象)会重新执行。
  useEffect(() => {
    let cancelled = false;
    const ok = <T,>(event: (value: T) => FirstRunEvent) => (value: T) => {
      if (!cancelled) dispatch(event(value));
    };
    const fail = (event: (error: unknown) => FirstRunEvent) => (error: unknown) => {
      if (!cancelled) dispatch(event(error));
    };
    if (state.step === "detecting") {
      productApi
        .status()
        .then(ok((status) => ({ type: "STATUS_RESOLVED", status })))
        .catch(fail((error) => ({ type: "STATUS_FAILED", error })));
    } else if (state.step === "planning") {
      productApi
        .plan("install")
        .then(ok((plan) => ({ type: "PLAN_RESOLVED", plan })))
        .catch(fail((error) => ({ type: "PLAN_FAILED", error })));
    } else if (state.step === "installing") {
      productApi
        .execute("install")
        .then(ok((result) => ({ type: "INSTALL_RESOLVED", result })))
        .catch(fail((error) => ({ type: "INSTALL_FAILED", error })));
    } else if (state.step === "redeploying") {
      productApi
        .redeploy()
        .then(ok(() => ({ type: "REDEPLOY_RESOLVED" })))
        // 自动部署不可用不是失败:展示精确手动指引(官方途径)。
        .catch(fail((error) => ({ type: "REDEPLOY_MANUAL", error })));
    } else if (state.step === "verifying") {
      productApi
        .status()
        .then(ok((status) => ({ type: "VERIFY_RESOLVED", status })))
        .catch(fail((error) => ({ type: "VERIFY_FAILED", error })));
    }
    return () => {
      cancelled = true;
    };
  }, [state]);

  // 终局:持久化最小记录并收尾(仅一次)。
  useEffect(() => {
    if (!isTerminal(state) || outcomeHandled.current) return;
    outcomeHandled.current = true;
    writeOnboarding(state.outcome);
    if (state.outcome === "completed" && state.startTraining) {
      onStartTraining(STARTER_TRAINING_PATH[0].mode);
    }
    setDismissed(true);
  }, [state, onStartTraining]);

  if (record || dismissed) return null;

  const copyDiagnostics = async () => {
    try {
      const report = await productApi.diagnostics();
      await navigator.clipboard.writeText(report);
      setDiagnosticsCopied(true);
    } catch {
      // 复制失败不打断向导;诊断仍可在控制中心获取。
    }
  };

  const schemaLabel = (schema: SchemaChoice) =>
    schema === "flow" ? t("firstRun.schemaFlow") : t("firstRun.schemaStatic");

  // 去控制中心 = 结束向导(按跳过持久化),同时切换到控制中心视图。
  const openControlCenter = () => {
    if (!isTerminal(state)) dispatch({ type: "SKIP" });
    onOpenControlCenter();
  };

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        // X / Esc 关闭 = 跳过(持久化),下次启动不再弹出。
        if (!open && !isTerminal(state)) dispatch({ type: "SKIP" });
      }}
    >
      <DialogContent aria-describedby="first-run-description">
        <StageHint state={state} />
        <StepView
          state={state}
          dispatch={dispatch}
          t={t}
          schemaLabel={schemaLabel}
          diagnosticsCopied={diagnosticsCopied}
          onCopyDiagnostics={copyDiagnostics}
          onOpenControlCenter={openControlCenter}
        />
      </DialogContent>
    </Dialog>
  );
}

/** 顶部阶段提示(文本,不依赖颜色)。 */
function StageHint({ state }: { state: FirstRunState }) {
  const { t } = useI18n();
  const stageKey = ((): I18nKey | null => {
    switch (state.step) {
      case "welcome":
        return null;
      case "detecting":
      case "detect-failed":
      case "rime-missing":
        return "firstRun.stageDetect";
      case "already-healthy":
      case "offer-update":
      case "offer-repair":
      case "schema-choice":
      case "planning":
      case "plan-preview":
      case "plan-failed":
        return "firstRun.stagePlan";
      case "installing":
      case "install-failed":
      case "redeploying":
      case "redeploy-manual":
        return "firstRun.stageInstall";
      case "verifying":
      case "verify-failed":
        return "firstRun.stageVerify";
      case "input-check":
      case "training-offer":
        return "firstRun.stageFinish";
      case "completed":
        return null;
    }
  })();
  if (!stageKey) return null;
  return (
    <p
      className="mb-3 text-xs font-medium uppercase tracking-wide text-muted-foreground"
      aria-label={t(stageKey)}
    >
      {t(stageKey)}
    </p>
  );
}

interface StepViewProps {
  state: FirstRunState;
  dispatch: (event: FirstRunEvent) => void;
  t: Translate;
  schemaLabel: (schema: SchemaChoice) => string;
  diagnosticsCopied: boolean;
  onCopyDiagnostics: () => void;
  onOpenControlCenter: () => void;
}

function StepView({ state, dispatch, t, schemaLabel, diagnosticsCopied, onCopyDiagnostics, onOpenControlCenter }: StepViewProps) {
  const busy =
    state.step === "detecting" ||
    state.step === "planning" ||
    state.step === "installing" ||
    state.step === "redeploying" ||
    state.step === "verifying";
  const error = "error" in state ? state.error : null;
  const status = "status" in state ? state.status : null;

  return (
    <>
      <DialogTitle>{t(TITLE_KEYS[state.step] ?? "firstRun.titleWelcome")}</DialogTitle>
      <DialogDescription id="first-run-description">
        {t(DESC_KEYS[state.step] ?? "firstRun.descWelcome")}
      </DialogDescription>

      {busy && (
        <p className="mt-4 flex items-center gap-2 text-sm text-muted-foreground" role="status">
          <Loader2 className="size-4 animate-spin" aria-hidden />
          {t("firstRun.working")}
        </p>
      )}

      {error !== null && state.step !== "redeploy-manual" && (
        <p className="mt-4 rounded-lg bg-destructive/10 p-3 text-sm text-destructive" role="alert">
          {errorText(error, t)}
        </p>
      )}

      <div className="mt-4 space-y-3 text-sm">
        <StateDetails
          state={state}
          status={status}
          dispatch={dispatch}
          t={t}
          schemaLabel={schemaLabel}
        />
      </div>

      <DialogFooter>
        {!isTerminal(state) && (
          <Button variant="ghost" className="min-h-11" onClick={() => dispatch({ type: "SKIP" })}>
            {t("firstRun.skip")}
          </Button>
        )}
        <StepActions
          state={state}
          dispatch={dispatch}
          t={t}
          diagnosticsCopied={diagnosticsCopied}
          onCopyDiagnostics={onCopyDiagnostics}
          onOpenControlCenter={onOpenControlCenter}
        />
      </DialogFooter>
    </>
  );
}

/** 步骤主体(按状态渲染细节)。 */
function StateDetails({
  state,
  status,
  dispatch,
  t,
  schemaLabel,
}: {
  state: FirstRunState;
  status: ProductStatusDto | null;
  dispatch: (event: FirstRunEvent) => void;
  t: Translate;
  schemaLabel: (schema: SchemaChoice) => string;
}) {
  switch (state.step) {
    case "rime-missing":
      return (
        <div className="space-y-2">
          <p>{t("firstRun.rimeMissingGuidance")}</p>
          <ul className="list-disc space-y-1 pl-5">
            <li>{t("product.clientWeasel")}</li>
            <li>{t("product.clientSquirrel")}</li>
            <li>{t("product.clientFcitx5")}</li>
            <li>{t("product.clientIbus")}</li>
          </ul>
          <p className="text-muted-foreground">{t("firstRun.rimeMissingPrivacy")}</p>
        </div>
      );
    case "already-healthy":
      return (
        <div className="space-y-2">
          <p className="flex items-center gap-2">
            <CheckCircle2 className="size-4 text-primary" aria-hidden />
            {t("firstRun.healthySummary", {
              client: status ? t(CLIENT_LABELS[status.client]) : "",
              version: status?.install?.installed_version ?? status?.bundled_version ?? "",
            })}
          </p>
          <p className="text-muted-foreground">{t("firstRun.healthyNext")}</p>
        </div>
      );
    case "offer-update":
      return (
        <p>
          {t("firstRun.offerUpdateDetail", {
            installed: status?.install?.installed_version ?? t("firstRun.unknownVersion"),
            bundled: status?.bundled_version ?? t("firstRun.unknownVersion"),
          })}
        </p>
      );
    case "offer-repair":
      return <p>{t("firstRun.offerRepairDetail")}</p>;
    case "schema-choice":
      return (
        <div className="grid gap-3">
          <Button
            variant="outline"
            className="min-h-11 justify-start text-start"
            onClick={() => dispatch({ type: "SCHEMA_CHOSEN", schema: "flow" })}
          >
            <span>
              <span className="block font-medium">{t("firstRun.schemaFlow")}</span>
              <span className="block text-xs font-normal text-muted-foreground">
                {t("firstRun.schemaFlowDesc")}
              </span>
            </span>
          </Button>
          <Button
            variant="outline"
            className="min-h-11 justify-start text-start"
            onClick={() => dispatch({ type: "SCHEMA_CHOSEN", schema: "static" })}
          >
            <span>
              <span className="block font-medium">{t("firstRun.schemaStatic")}</span>
              <span className="block text-xs font-normal text-muted-foreground">
                {t("firstRun.schemaStaticDesc")}
              </span>
            </span>
          </Button>
          <p className="text-xs text-muted-foreground">{t("firstRun.schemaBothInstalled")}</p>
        </div>
      );
    case "plan-preview":
      return (
        <div className="space-y-2">
          <p className="font-medium">{t("firstRun.planTitle")}</p>
          <ul className="max-h-40 space-y-1 overflow-auto rounded-lg border border-border p-2 font-mono text-xs">
            {state.plan.actions.map((action) => (
              <li key={`${action.kind}:${action.file}`}>
                {t(PLAN_ACTION_LABELS[action.kind])} · {action.file}
                {action.backup ? ` → ${action.backup}` : ""}
              </li>
            ))}
            {state.plan.actions.length === 0 && <li>{t("firstRun.planEmpty")}</li>}
          </ul>
          {state.plan.notes.map((note) => (
            <p key={note} className="text-xs text-muted-foreground">
              {note}
            </p>
          ))}
          <p className="text-xs text-muted-foreground">{t("firstRun.planSafety")}</p>
          <p className="text-xs text-muted-foreground">{t("firstRun.chosenSchema", { schema: schemaLabel(state.schema) })}</p>
        </div>
      );
    case "redeploy-manual":
      return (
        <div className="space-y-2">
          <p className="rounded-lg bg-muted p-3">{state.result.redeploy_guidance}</p>
          <p className="text-muted-foreground">{t("firstRun.redeployManualHint")}</p>
        </div>
      );
    case "input-check":
      return (
        <div className="space-y-2">
          <p className="flex items-center gap-2">
            <CheckCircle2 className="size-4 text-primary" aria-hidden />
            {t("firstRun.verifyPassed", {
              files: `${state.status.install?.installed_files ?? 0}/${state.status.install?.total_files ?? 0}`,
            })}
          </p>
          <p>{t("firstRun.inputCheckIntro")}</p>
          <ul className="rounded-lg border border-border p-3 font-mono text-xs">
            <li>womf → 我们</li>
            <li>uijm → 时间</li>
          </ul>
          <p className="text-xs text-muted-foreground">{t("firstRun.inputCheckNote")}</p>
        </div>
      );
    case "training-offer":
      return (
        <div className="space-y-2">
          <ol className="list-decimal space-y-1 pl-5">
            {STARTER_TRAINING_PATH.map((item) => (
              <li key={item.mode}>{t(item.label)}</li>
            ))}
          </ol>
          <p className="text-muted-foreground">{t("firstRun.trainingOfferNote")}</p>
        </div>
      );
    default:
      return null;
  }
}

/** 步骤主按钮(次按钮在前,主按钮在后)。 */
function StepActions({
  state,
  dispatch,
  t,
  diagnosticsCopied,
  onCopyDiagnostics,
  onOpenControlCenter,
}: {
  state: FirstRunState;
  dispatch: (event: FirstRunEvent) => void;
  t: Translate;
  diagnosticsCopied: boolean;
  onCopyDiagnostics: () => void;
  onOpenControlCenter: () => void;
}) {
  const copyButton = (
    <Button variant="outline" className="min-h-11" onClick={onCopyDiagnostics}>
      <ClipboardCopy className="size-4" aria-hidden />
      {diagnosticsCopied ? t("firstRun.copied") : t("firstRun.copyDiagnostics")}
    </Button>
  );
  /** 去控制中心人工处理:视为跳过向导(持久化),不再自动弹出。 */
  const controlCenterButton = (
    <Button variant="outline" className="min-h-11" onClick={onOpenControlCenter}>
      {t("firstRun.openControlCenter")}
    </Button>
  );
  switch (state.step) {
    case "welcome":
      return (
        <Button className="min-h-11" onClick={() => dispatch({ type: "START" })}>
          {t("firstRun.begin")}
        </Button>
      );
    case "detecting":
    case "planning":
    case "installing":
    case "redeploying":
    case "verifying":
      return null;
    case "detect-failed":
      return (
        <>
          {hasDiagnostics(state.error) && copyButton}
          <Button className="min-h-11" onClick={() => dispatch({ type: "RETRY" })}>
            {t("common.retry")}
          </Button>
        </>
      );
    case "rime-missing":
      return (
        <Button className="min-h-11" onClick={() => dispatch({ type: "RETRY" })}>
          {t("firstRun.redetect")}
        </Button>
      );
    case "already-healthy":
      return (
        <>
          {controlCenterButton}
          <Button variant="outline" className="min-h-11" onClick={() => dispatch({ type: "PROCEED" })}>
            {t("firstRun.goTraining")}
          </Button>
          <Button className="min-h-11" onClick={() => dispatch({ type: "FINISH" })}>
            {t("firstRun.enterApp")}
          </Button>
        </>
      );
    case "offer-update":
    case "offer-repair":
      return (
        <Button className="min-h-11" onClick={() => dispatch({ type: "PLAN_REQUESTED" })}>
          {t("firstRun.makePlan")}
        </Button>
      );
    case "plan-failed":
      return (
        <Button className="min-h-11" onClick={() => dispatch({ type: "RETRY" })}>
          {t("common.retry")}
        </Button>
      );
    case "plan-preview":
      return (
        <>
          <Button variant="outline" className="min-h-11" onClick={() => dispatch({ type: "BACK" })}>
            {t("common.back")}
          </Button>
          <Button
            variant="outline"
            className="min-h-11"
            onClick={() => dispatch({ type: "PLAN_REQUESTED" })}
          >
            {t("firstRun.refreshPlan")}
          </Button>
          <Button className="min-h-11" onClick={() => dispatch({ type: "CONFIRM_INSTALL" })}>
            {t("firstRun.confirmInstall")}
          </Button>
        </>
      );
    case "install-failed":
      return (
        <>
          {copyButton}
          <Button className="min-h-11" onClick={() => dispatch({ type: "RETRY" })}>
            {t("common.retry")}
          </Button>
        </>
      );
    case "redeploy-manual":
      return (
        <Button className="min-h-11" onClick={() => dispatch({ type: "PROCEED" })}>
          {t("firstRun.afterRedeploy")}
        </Button>
      );
    case "verify-failed":
      return (
        <>
          {copyButton}
          {controlCenterButton}
          <Button className="min-h-11" onClick={() => dispatch({ type: "RETRY" })}>
            {t("firstRun.reverify")}
          </Button>
        </>
      );
    case "input-check":
      return (
        <Button className="min-h-11" onClick={() => dispatch({ type: "PROCEED" })}>
          {t("firstRun.next")}
        </Button>
      );
    case "training-offer":
      return (
        <>
          <Button variant="outline" className="min-h-11" onClick={() => dispatch({ type: "FINISH" })}>
            {t("firstRun.enterApp")}
          </Button>
          <Button className="min-h-11" onClick={() => dispatch({ type: "PROCEED" })}>
            {t("firstRun.startTraining")}
          </Button>
        </>
      );
    case "completed":
      return null;
    default:
      return null;
  }
}

/** 诊断是否可复制:浏览器降级(桌面不可用)没有可查询的诊断。 */
function hasDiagnostics(error: unknown): boolean {
  return !(error instanceof CommandError && error.code === "desktop_unavailable");
}

const TITLE_KEYS: Partial<Record<FirstRunState["step"], I18nKey>> = {
  welcome: "firstRun.titleWelcome",
  detecting: "firstRun.titleDetecting",
  "detect-failed": "firstRun.titleDetectFailed",
  "rime-missing": "firstRun.titleRimeMissing",
  "already-healthy": "firstRun.titleAlreadyHealthy",
  "offer-update": "firstRun.titleOfferUpdate",
  "offer-repair": "firstRun.titleOfferRepair",
  "schema-choice": "firstRun.titleSchema",
  planning: "firstRun.titleWorking",
  "plan-preview": "firstRun.titlePlan",
  "plan-failed": "firstRun.titlePlanFailed",
  installing: "firstRun.titleInstalling",
  "install-failed": "firstRun.titleInstallFailed",
  redeploying: "firstRun.titleRedeploying",
  "redeploy-manual": "firstRun.titleRedeployManual",
  verifying: "firstRun.titleVerifying",
  "verify-failed": "firstRun.titleVerifyFailed",
  "input-check": "firstRun.titleInputCheck",
  "training-offer": "firstRun.titleTrainingOffer",
  completed: "firstRun.titleWelcome",
};

const DESC_KEYS: Partial<Record<FirstRunState["step"], I18nKey>> = {
  welcome: "firstRun.descWelcome",
  detecting: "firstRun.descWorking",
  "detect-failed": "firstRun.descDetectFailed",
  "rime-missing": "firstRun.descRimeMissing",
  "already-healthy": "firstRun.descAlreadyHealthy",
  "offer-update": "firstRun.descOfferUpdate",
  "offer-repair": "firstRun.descOfferRepair",
  "schema-choice": "firstRun.descSchema",
  planning: "firstRun.descWorking",
  "plan-preview": "firstRun.descPlanPreview",
  "plan-failed": "firstRun.descPlanFailed",
  installing: "firstRun.descInstalling",
  "install-failed": "firstRun.descInstallFailed",
  redeploying: "firstRun.descRedeploying",
  "redeploy-manual": "firstRun.descRedeployManual",
  verifying: "firstRun.descVerifying",
  "verify-failed": "firstRun.descVerifyFailed",
  "input-check": "firstRun.descInputCheck",
  "training-offer": "firstRun.descTrainingOffer",
  completed: "firstRun.descWelcome",
};
