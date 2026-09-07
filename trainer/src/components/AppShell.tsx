/**
 * 应用壳:桌面侧边栏 + 移动端底部导航 + 当前视图。
 *
 * 无路由库;视图切换是即时的内部状态。
 * 「练这些」(错题)与「开始练习」(今日)通过预设参数跳入练习视图。
 */

import { useCallback, useEffect, useRef, useState } from "react";
import {
  ChartColumn,
  GraduationCap,
  Grid3x3,
  House,
  Keyboard,
  Package,
  RotateCcw,
  Settings,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { consumeBack } from "@/lib/back-handler";
import type { I18nKey } from "@/lib/i18n";
import { useI18n } from "@/lib/use-i18n";
import type { TrainingItem } from "@/lib/trainer-index";
import { DashboardView } from "@/features/dashboard/DashboardView";
import { PracticeSetupView } from "@/features/practice/PracticeSetupView";
import { WeaknessCenter } from "@/features/review/ReviewView";
import { StatsView } from "@/features/stats/StatsView";
import { ReferenceView } from "@/features/reference/ReferenceView";
import { ControlCenterView } from "@/features/product/ControlCenterView";
import { FirstRunWizard } from "@/features/first-run/FirstRunWizard";
import { clearOnboarding } from "@/features/first-run/onboarding";
import { LearnView } from "@/features/learn/LearnView";
import { SettingsView } from "@/features/settings/SettingsView";
import type { PracticeMode } from "@/features/practice/types";

export type ViewKey =
  | "today"
  | "practice"
  | "review"
  | "stats"
  | "reference"
  | "learn"
  | "product"
  | "settings";

const NAV_ITEMS: { key: ViewKey; label: I18nKey; icon: typeof House }[] = [
  { key: "today", label: "nav.today", icon: House },
  { key: "practice", label: "nav.practice", icon: Keyboard },
  { key: "review", label: "nav.review", icon: RotateCcw },
  { key: "stats", label: "nav.stats", icon: ChartColumn },
  { key: "reference", label: "nav.reference", icon: Grid3x3 },
  { key: "learn", label: "nav.learn", icon: GraduationCap },
  { key: "product", label: "nav.product", icon: Package },
  { key: "settings", label: "nav.settings", icon: Settings },
];

const ROOT_VIEW: ViewKey = "today";
const EXIT_GUARD_MS = 2000;

export function AppShell() {
  const { t } = useI18n();
  const [view, setView] = useState<ViewKey>(ROOT_VIEW);
  /** 跳入练习时预选的模式(今日的模式卡片)。 */
  const [presetMode, setPresetMode] = useState<PracticeMode | null>(null);
  /** 跳入练习时指定的复习条目(错题「练这些」)。 */
  const [reviewEntries, setReviewEntries] = useState<TrainingItem[] | null>(null);
  /** 首次启动向导的重新运行信号(设置页触发,自增)。 */
  const [onboardingReopen, setOnboardingReopen] = useState(0);
  /** 根级退出保护提示(「再按一次退出」)。 */
  const [exitToast, setExitToast] = useState(false);
  /** 导航上退轨迹:history 每压入一层,这里记一个来源视图。 */
  const trailRef = useRef<ViewKey[]>([]);
  const exitIntentRef = useRef(0);

  /** 应用内导航:压入 history,让 Android 返回手势/浏览器后退逐层上退。 */
  const navigate = useCallback((next: ViewKey) => {
    setView((current) => {
      if (next === current) return current;
      trailRef.current.push(current);
      window.history.pushState({ xhupNav: trailRef.current.length }, "");
      return next;
    });
  }, []);

  // Android 返回 / 浏览器后退:嵌套屏先认领,否则沿轨迹上退;
  // 到根后第一次返回给退出提示,2 秒内再次返回才真正退出。
  useEffect(() => {
    window.history.replaceState({ xhupNav: 0 }, "");
    const onPop = () => {
      if (consumeBack()) {
        // 嵌套屏消费了本次返回:恢复当前层的历史条目。
        window.history.pushState({ xhupNav: trailRef.current.length }, "");
        return;
      }
      if (trailRef.current.length > 0) {
        const previous = trailRef.current.pop() as ViewKey;
        exitIntentRef.current = 0;
        setExitToast(false);
        setView(previous);
        return;
      }
      const now = Date.now();
      if (now - exitIntentRef.current < EXIT_GUARD_MS) {
        exitIntentRef.current = 0;
        setExitToast(false);
        // 不再压入历史:此时栈已空,系统按默认行为收起/退出应用。
        window.history.back();
        return;
      }
      exitIntentRef.current = now;
      setExitToast(true);
      window.history.pushState({ xhupNav: 0 }, "");
    };
    window.addEventListener("popstate", onPop);
    return () => window.removeEventListener("popstate", onPop);
  }, []);

  // 退出提示 2 秒后自动消失。
  useEffect(() => {
    if (!exitToast) return;
    const timer = setTimeout(() => setExitToast(false), EXIT_GUARD_MS);
    return () => clearTimeout(timer);
  }, [exitToast]);

  const goPractice = (mode: PracticeMode) => {
    setPresetMode(mode);
    setReviewEntries(null);
    navigate("practice");
  };

  const goReviewPractice = (entries: TrainingItem[]) => {
    setReviewEntries(entries);
    setPresetMode(null);
    navigate("practice");
  };

  /** 会话结束回今日:清空轨迹,返回手势回到根级保护。 */
  const exitToToday = useCallback(() => {
    trailRef.current = [];
    exitIntentRef.current = 0;
    setView(ROOT_VIEW);
  }, []);

  return (
    <div className="min-h-svh bg-background text-foreground">
      <DesktopSidebar active={view} onNavigate={navigate} />
      <main className="mx-auto w-full max-w-5xl px-4 pb-24 pt-[calc(1rem+env(safe-area-inset-top))] sm:px-6 md:pb-10 md:pl-60 md:pt-[calc(2rem+env(safe-area-inset-top))]">
        {view === "today" && (
          <DashboardView
            onStartPractice={goPractice}
            onShowReview={() => navigate("review")}
            onOpenLearn={() => navigate("learn")}
          />
        )}
        {view === "practice" && (
          <PracticeSetupView
            presetMode={presetMode}
            reviewEntries={reviewEntries}
            onPresetConsumed={() => {
              setPresetMode(null);
              setReviewEntries(null);
            }}
            onExitToToday={exitToToday}
          />
        )}
        {view === "review" && <WeaknessCenter onPracticeEntries={goReviewPractice} />}
        {view === "stats" && <StatsView />}
        {view === "reference" && <ReferenceView />}
        {view === "learn" && <LearnView onStartPractice={goPractice} />}
        {view === "product" && <ControlCenterView />}
        {view === "settings" && (
          <SettingsView
            onRerunOnboarding={() => {
              clearOnboarding();
              setOnboardingReopen((n) => n + 1);
            }}
          />
        )}
      </main>
      {/* 练习流程是沉浸式屏幕:隐藏底部标签栏,返回手势负责上退。 */}
      {view !== "practice" && <MobileBottomNav active={view} onNavigate={navigate} />}
      {exitToast && (
        <div
          role="status"
          className="fixed bottom-[calc(env(safe-area-inset-bottom)+5rem)] left-1/2 z-50 -translate-x-1/2 rounded-full bg-foreground/90 px-4 py-2 text-sm text-background shadow-lg md:bottom-[calc(env(safe-area-inset-bottom)+1.5rem)]"
        >
          {t("app.exitToast")}
        </div>
      )}
      {/* 首次启动向导:完成/跳过后不再出现;不影响常规导航。 */}
      <FirstRunWizard
        reopenSignal={onboardingReopen}
        onStartTraining={(mode) => {
          setPresetMode(mode);
          setReviewEntries(null);
          setView("practice");
        }}
        onOpenControlCenter={() => navigate("product")}
      />
    </div>
  );
}

function DesktopSidebar({
  active,
  onNavigate,
}: {
  active: ViewKey;
  onNavigate: (view: ViewKey) => void;
}) {
  const { t } = useI18n();
  return (
    <aside className="fixed inset-y-0 left-0 hidden w-52 flex-col border-r border-border bg-card/50 px-3 py-5 md:flex">
      <div className="px-2">
        <p className="text-base font-semibold tracking-tight">{t("app.name")}</p>
        <p className="mt-0.5 text-xs text-muted-foreground">{t("app.subtitle")}</p>
      </div>
      <nav className="mt-6 flex flex-col gap-1" aria-label={t("nav.main")}>
        {NAV_ITEMS.map((item) => (
          <button
            key={item.key}
            type="button"
            onClick={() => onNavigate(item.key)}
            aria-current={active === item.key ? "page" : undefined}
            className={cn(
              "flex min-h-11 items-center gap-3 rounded-lg px-3 text-sm font-medium transition-colors focus-visible:outline-2 focus-visible:outline-ring",
              active === item.key
                ? "bg-primary/10 text-primary"
                : "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
            )}
          >
            <item.icon className="size-4" aria-hidden />
            {t(item.label)}
          </button>
        ))}
      </nav>
    </aside>
  );
}

function MobileBottomNav({
  active,
  onNavigate,
}: {
  active: ViewKey;
  onNavigate: (view: ViewKey) => void;
}) {
  const { t } = useI18n();
  return (
    <nav
      className="fixed inset-x-0 bottom-0 z-40 flex border-t border-border bg-card pb-[env(safe-area-inset-bottom)] md:hidden"
      aria-label={t("nav.main")}
    >
      {NAV_ITEMS.map((item) => (
        <button
          key={item.key}
          type="button"
          onClick={() => onNavigate(item.key)}
          aria-current={active === item.key ? "page" : undefined}
          className={cn(
            "flex min-h-14 flex-1 flex-col items-center justify-center gap-0.5 text-[11px] font-medium transition-colors focus-visible:outline-2 focus-visible:outline-ring",
            active === item.key ? "text-primary" : "text-muted-foreground",
          )}
        >
          <item.icon className="size-5" aria-hidden />
          {t(item.label)}
        </button>
      ))}
    </nav>
  );
}
