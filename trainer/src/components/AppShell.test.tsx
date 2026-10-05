import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { buildTrainerIndex, type TrainerDataset } from "@xhup/trainer-core";
import { TrainerIndexProvider } from "@/lib/trainer-context";
import { resetTrainerStore } from "@/stores/trainer-store";
import { AppShell } from "./AppShell";

const dataset: TrainerDataset = {
  schemaVersion: 4,
  packageVersion: "2.0.0-rc.3",
  entries: [{ char: "阿", code: "aaed", length: 4, readings: ["a"], frequencyScore: 100,
    rimeWeight: 1, scope: "core", codeSource: "canonical-reading-shape", sources: [], statuses: [] }],
  words: [], level1Shortcuts: [], primaryShortcuts: [], fixedFirstShortcuts: [], sentences: [],
  doublePinyin: { initials: [], finals: [], zeroInitials: [] },
};

beforeEach(() => resetTrainerStore());
afterEach(() => Reflect.deleteProperty(window, "__TAURI_INTERNALS__"));

describe("training-only application", () => {
  it("starts without an installer and keeps all seven training destinations without native calls", async () => {
    const invoke = vi.fn().mockRejectedValue(new Error("no native application commands"));
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: { invoke }, configurable: true });
    render(<TrainerIndexProvider index={buildTrainerIndex(dataset)}><AppShell /></TrainerIndexProvider>);
    const navigation = within(screen.getAllByRole("navigation", { name: "主导航" })[0]);
    const labels = ["今日", "练习", "错题", "统计", "键位", "学习", "设置"];
    expect(navigation.getAllByRole("button").map(button => button.textContent)).toEqual(labels);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "输入法" })).not.toBeInTheDocument();
    const user = userEvent.setup();
    for (const label of labels) await user.click(navigation.getByRole("button", { name: label }));
    expect(screen.getByRole("button", { name: "导出进度备份" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "安装" })).not.toBeInTheDocument();
    expect(screen.queryByText("输入法学习数据")).not.toBeInTheDocument();
    expect(screen.getByTestId("training-data-version")).toHaveTextContent(dataset.packageVersion);
    expect(invoke).not.toHaveBeenCalled();
  });
});
