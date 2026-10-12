// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render } from "@testing-library/react";
import type { HTMLAttributes } from "react";
import { createElement } from "react";

const host = vi.hoisted(() => ({ storage: new Map<string, string>(), hide: () => {} }));
vi.mock("@tarojs/taro", () => ({
  useDidHide: (callback: () => void) => { host.hide = callback; },
  default: {
    getStorageSync: (key: string) => host.storage.get(key) ?? "",
    setStorageSync: (key: string, value: string) => { host.storage.set(key, value); },
    removeStorageSync: (key: string) => { host.storage.delete(key); },
    getCurrentInstance: () => ({ router: { params: { mode: "double" } } }),
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

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(100000);
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
});
