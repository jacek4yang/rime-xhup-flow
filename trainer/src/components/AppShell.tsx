/**
 * 应用壳:桌面侧边栏 + 移动端底部导航 + 当前视图。
 *
 * 无路由库;视图切换是即时的内部状态。
 * 「练这些」(错题)与「开始练习」(今日)通过预设参数跳入练习视图。
 */

import { useState } from "react";
import {
  Grid3x3,
  House,
  Keyboard,
  RotateCcw,
  Settings,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type { TrainingItem } from "@/lib/trainer-index";
import { DashboardView } from "@/features/dashboard/DashboardView";
import { PracticeSetupView } from "@/features/practice/PracticeSetupView";
import { ReviewView } from "@/features/review/ReviewView";
import { ReferenceView } from "@/features/reference/ReferenceView";
import { SettingsView } from "@/features/settings/SettingsView";
import type { PracticeMode } from "@/features/practice/types";

export type ViewKey = "today" | "practice" | "review" | "reference" | "settings";

const NAV_ITEMS: { key: ViewKey; label: string; icon: typeof House }[] = [
  { key: "today", label: "今日", icon: House },
  { key: "practice", label: "练习", icon: Keyboard },
  { key: "review", label: "错题", icon: RotateCcw },
  { key: "reference", label: "键位", icon: Grid3x3 },
  { key: "settings", label: "设置", icon: Settings },
];

export function AppShell() {
  const [view, setView] = useState<ViewKey>("today");
  /** 跳入练习时预选的模式(今日的模式卡片)。 */
  const [presetMode, setPresetMode] = useState<PracticeMode | null>(null);
  /** 跳入练习时指定的复习条目(错题「练这些」)。 */
  const [reviewEntries, setReviewEntries] = useState<TrainingItem[] | null>(null);

  const goPractice = (mode: PracticeMode) => {
    setPresetMode(mode);
    setReviewEntries(null);
    setView("practice");
  };

  const goReviewPractice = (entries: TrainingItem[]) => {
    setReviewEntries(entries);
    setPresetMode(null);
    setView("practice");
  };

  return (
    <div className="min-h-svh bg-background text-foreground">
      <DesktopSidebar active={view} onNavigate={setView} />
      <main className="mx-auto w-full max-w-5xl px-4 pb-24 pt-4 sm:px-6 md:pb-10 md:pl-60 md:pt-8">
        {view === "today" && (
          <DashboardView
            onStartPractice={goPractice}
            onShowReview={() => setView("review")}
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
            onExitToToday={() => setView("today")}
          />
        )}
        {view === "review" && <ReviewView onPracticeEntries={goReviewPractice} />}
        {view === "reference" && <ReferenceView />}
        {view === "settings" && <SettingsView />}
      </main>
      <MobileBottomNav active={view} onNavigate={setView} />
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
  return (
    <aside className="fixed inset-y-0 left-0 hidden w-52 flex-col border-r border-border bg-card/50 px-3 py-5 md:flex">
      <div className="px-2">
        <p className="text-base font-semibold tracking-tight">XHUP Flow</p>
        <p className="mt-0.5 text-xs text-muted-foreground">小鹤音形训练</p>
      </div>
      <nav className="mt-6 flex flex-col gap-1">
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
            {item.label}
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
  return (
    <nav
      className="fixed inset-x-0 bottom-0 z-40 flex border-t border-border bg-card pb-[env(safe-area-inset-bottom)] md:hidden"
      aria-label="主导航"
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
          {item.label}
        </button>
      ))}
    </nav>
  );
}
