// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, within } from "@testing-library/react";
import type { HTMLAttributes } from "react";
import { createElement } from "react";

const host = vi.hoisted(() => ({
  storage: new Map<string, string>(), hide: () => {},
  params: { mode: "double" } as Record<string, string>,
}));
vi.mock("@tarojs/taro", () => ({
  useDidHide: (callback: () => void) => { host.hide = callback; },
  default: {
    getStorageSync: (key: string) => host.storage.get(key) ?? "",
    setStorageSync: (key: string, value: string) => { host.storage.set(key, value); },
    removeStorageSync: (key: string) => { host.storage.delete(key); },
    getCurrentInstance: () => ({ router: { params: host.params } }),
    vibrateShort: vi.fn(), redirectTo: vi.fn(), showModal: vi.fn(),
  },
}));
vi.mock("@tarojs/components", () => ({
  View: (props: HTMLAttributes<HTMLDivElement>) => createElement("div", props),
  Text: (props: HTMLAttributes<HTMLSpanElement>) => createElement("span", props),
  ScrollView: ({ scrollY: _scrollY, ...props }: HTMLAttributes<HTMLDivElement> & { scrollY?: boolean }) => createElement("div", props),
}));

import Session from "./index";
import { appStore } from "../../lib/store";
import { defaultAppState } from "../../lib/state-schema";
import { trainerIndex } from "../../lib/dataset";
import { emptyProgress } from "@xhup/trainer-core";

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(100000);
  host.params = { mode: "double" };
  vi.spyOn(Math, "random").mockReturnValue(0);
  appStore.resetProgress();
  appStore.updateSettings(defaultAppState().settings);
});
afterEach(() => { cleanup(); vi.useRealTimers(); vi.restoreAllMocks(); });

describe("真实小程序页面的答案授权", () => {
  it.each(["always", "on-delay", "on-error", "hidden"] as const)(
    "盲打/%s：等待、错键、后台不泄露；主动帮助才揭示", (hintMode) => {
      appStore.updateSettings({ hintMode });
      const { container, getByText, getAllByText } = render(<Session />);
      const target = container.querySelector(".session-target")!.textContent;
      const item = trainerIndex.pools["char-2"].find((candidate) => candidate.target === target)!;
      const noAnswer = () => {
        expect(container.querySelector(".session-reading")).toBeNull();
        expect(container.querySelector(".session-code-hint")).toBeNull();
        expect(container.querySelector(".teaching-code")).toBeNull();
        for (const element of container.querySelectorAll("*")) {
          for (const attribute of element.attributes) {
            if (/^(aria-|data-|title$)/.test(attribute.name)) {
              expect(attribute.value).not.toContain(item.primaryCode);
              for (const reading of item.readings) expect(attribute.value).not.toContain(reading);
            }
          }
        }
      };
      noAnswer();
      act(() => { vi.advanceTimersByTime(2500); });
      noAnswer();
      const wrong = [..."abcdefghijklmnopqrstuvwxyz"].find((key) =>
        item.acceptedCodes.every((code) => code[0] !== key))!;
      fireEvent.click(getByText(wrong.toUpperCase()));
      noAnswer();
      act(() => { host.hide(); vi.advanceTimersByTime(60000); });
      noAnswer();
      const helpButtons = getAllByText("查看解析（记为求助）");
      fireEvent.click(helpButtons[helpButtons.length - 1]);
      expect(container.querySelector(".teaching-code")).not.toBeNull();
      fireEvent.click(getByText("重试本题"));
      for (const key of item.primaryCode) fireEvent.click(getByText(key.toUpperCase()));
      const progress = appStore.getState().progress[item.id];
      expect(progress?.correct).toBe(0);
      expect(progress?.wrong).toBe(1);
    },
  );
  it("学习模式迟疑时钟按题重置，后台和手动恢复不累计暂停时间", () => {
    appStore.updateSettings({ practiceIntent: "learn", hintMode: "on-delay" });
    const { container, getByText } = render(<Session />);
    const target = container.querySelector(".session-target")!.textContent;
    const item = trainerIndex.pools["char-2"].find((candidate) => candidate.target === target)!;
    act(() => { vi.advanceTimersByTime(2001); });
    expect(container.querySelector(".session-code-hint")?.textContent).toBe(item.primaryCode);
    for (const key of item.primaryCode) fireEvent.click(getByText(key.toUpperCase()));
    act(() => { vi.advanceTimersByTime(1000); });
    expect(container.querySelector(".session-code-hint")).toBeNull();
    act(() => { vi.advanceTimersByTime(1000); host.hide(); });
    act(() => { vi.advanceTimersByTime(60000); });
    expect(container.querySelector(".session-code-hint")).toBeNull();
    fireEvent.click(within(container.querySelector(".pause-overlay") as HTMLElement).getByText("继续练习"));
    act(() => { vi.advanceTimersByTime(999); });
    expect(container.querySelector(".session-code-hint")).toBeNull();
    act(() => { vi.advanceTimersByTime(1); });
    expect(container.querySelector(".session-code-hint")).not.toBeNull();
  });
});

it.each(["level1", "sentence"] as const)("到期 %s 从复习路由可触屏完成且保存独立回忆", (kind) => {
  host.params = { mode: "mixed", src: "weak" };
  const item = [...trainerIndex.byId.values()].find((entry) => entry.kind === kind &&
    (kind !== "level1" || entry.alternateCode?.startsWith(entry.primaryCode)))!;
  appStore.applyBackup({ progress: {
    [item.id]: { ...emptyProgress(), attempts: 2, correct: 2, mastery: 100 },
  }, daily: {}, keyErrors: {} });
  const { container, getByText } = render(<Session />);
  expect(container.querySelector(".session-target")?.textContent).toBe(item.target);
  expect(container.querySelector(".session-reading")).toBeNull();
  for (const key of item.primaryCode) fireEvent.click(getByText(key.toUpperCase()));
  if (kind === "level1") fireEvent.click(getByText(/^确认/));
  const progress = appStore.getState().progress[item.id]!;
  expect(progress.attempts).toBe(3);
  expect(progress.review).toMatchObject({
    level: 1, lastResult: "recalled", lastIndependentAt: 100000,
    dueAt: 86500000, usedAnswerHint: false, routeUsed: "primary",
  });
});
