// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { createElement } from "react";
import type { HTMLAttributes } from "react";
import { readFileSync } from "node:fs";
import { brotliDecompressSync } from "node:zlib";
import { emptyProgress } from "@xhup/trainer-core";

const host = vi.hoisted(() => ({
  storage: new Map<string, string>(),
  read: vi.fn(),
}));
vi.mock("@tarojs/taro", () => ({
  useDidHide: () => {},
  default: {
    getStorageSync: (key: string) => host.storage.get(key) ?? "",
    setStorageSync: (key: string, value: string) => { host.storage.set(key, value); },
    removeStorageSync: (key: string) => { host.storage.delete(key); },
    getCurrentInstance: () => ({ router: { params: { mode: "mixed", src: "weak" } } }),
    getFileSystemManager: () => ({ readCompressedFile: host.read }),
    vibrateShort: vi.fn(), redirectTo: vi.fn(), showModal: vi.fn(),
  },
}));
vi.mock("@tarojs/components", () => ({
  View: (props: HTMLAttributes<HTMLDivElement>) => createElement("div", props),
  Text: (props: HTMLAttributes<HTMLSpanElement>) => createElement("span", props),
  ScrollView: ({ scrollY: _scrollY, ...props }: HTMLAttributes<HTMLDivElement> & { scrollY?: boolean }) => createElement("div", props),
}));

import { TrainerDataProvider } from "./trainer-data";
import Session from "../pages/session/index";
import { appStore } from "./store";

afterEach(cleanup);

it("读取失败不启动残缺练习;回调式文件API重试后能盲打完成完整规范去字全码", async () => {
  const bytes = brotliDecompressSync(readFileSync("src/data/generated/trainer.json.br"));
  const data = Uint8Array.from(bytes).buffer;
  // Native filesystem methods may return void, unlike Taro's Promise typings.
  host.read.mockImplementationOnce((options) => {
    queueMicrotask(() => options.fail?.({ errMsg: "readCompressedFile:fail" }));
  }).mockImplementationOnce((options) => {
    queueMicrotask(() => options.success?.({ data: new ArrayBuffer(1), errMsg: "readCompressedFile:ok" }));
  }).mockImplementationOnce((options) => {
    queueMicrotask(() => options.success?.({ data, errMsg: "readCompressedFile:ok" }));
  });
  appStore.applyBackup({ progress: {
    "去:quts": { ...emptyProgress(), attempts: 1, wrong: 1 },
  }, daily: {}, keyErrors: {} });
  const { container, findByRole, getByText, getByRole } = render(
    <TrainerDataProvider><main aria-label="原生页面"><Session /></main></TrainerDataProvider>,
  );
  // Taro mounts a page root below App; loading/error UI must not remove it.
  expect(getByRole("main", { name: "原生页面" }).isConnected).toBe(true);
  const retry = await findByRole("button", { name: "重试" });
  expect(container.querySelector(".session-target")).toBeNull();
  expect(appStore.getState().progress["去:quts"].attempts).toBe(1);
  fireEvent.click(retry);
  await findByRole("alert");
  expect(container.querySelector(".session-target")).toBeNull();
  expect(appStore.getState().progress["去:quts"].attempts).toBe(1);
  fireEvent.click(await findByRole("button", { name: "重试" }));
  await waitFor(() => expect(container.querySelector(".session-target")?.textContent).toBe("去"));
  expect(container.querySelector(".session-reading")).toBeNull();
  expect(container.querySelector(".session-code-hint")).toBeNull();
  expect(container.innerHTML).not.toContain("quts");
  for (const key of "QUTS") fireEvent.click(getByText(key));
  const progress = appStore.getState().progress["去:quts"];
  expect(progress.attempts).toBe(2);
  expect(progress.correct).toBe(1);
  expect(progress.wrong).toBe(1);
  expect(progress.review).toMatchObject({ level: 1, lastResult: "recalled", usedAnswerHint: false, routeUsed: "primary" });
});
