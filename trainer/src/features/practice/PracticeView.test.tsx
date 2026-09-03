/**
 * 练习流程的组件级测试:以用户可见行为为准(码位格、提示、暂停、小结)。
 */

import { beforeEach, describe, expect, it } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { TrainerEntry } from "@/lib/trainer-data";
import type { TrainingItem } from "@/lib/trainer-index";
import { buildTrainerIndex, charItem, type TrainerIndex } from "@/lib/trainer-index";
import { TrainerIndexProvider } from "@/lib/trainer-context";
import { resetTrainerStore, useTrainerStore } from "@/stores/trainer-store";
import { PracticeSetupView } from "./PracticeSetupView";
import { PracticeView } from "./PracticeView";
import type { PracticeConfig } from "./PracticeSetupView";

const ENTRIES: TrainerEntry[] = [
  {
    char: "行",
    code: "xk",
    length: 2,
    readings: ["xing", "hang"],
    frequencyScore: 100,
    rimeWeight: 1,
  },
];

function fixtureIndex(): TrainerIndex {
  return buildTrainerIndex({
    schemaVersion: 2,
    packageVersion: "0.1.0",
    entries: ENTRIES,
    words: [{ word: "我们", code: "womf", length: 4, charCount: 2, rimeWeight: 9 }],
    level1Shortcuts: [{ key: "q", char: "去" }],
    wordShortcuts: [{ word: "时间", fullCode: "uijm", shortcutCode: "uij", mode: "FF" }],
    fixedFirstShortcuts: [{ word: "发展", fullCode: "favj", shortcutCode: "faj", mode: "FFI" }],
    twoKeyShortcuts: [{ word: "记得", fullCode: "jide", shortcutCode: "jd", mode: "II" }],
    sentences: [{ text: "我们时间", code: "womfuijm", components: ["我们", "时间"] }],
    doublePinyin: { initials: [], finals: [], zeroInitials: [] },
  });
}

const ITEMS: TrainingItem[] = ENTRIES.map(charItem);

function noop() {}

function renderPractice(config?: Partial<PracticeConfig>) {
  const fullConfig: PracticeConfig = {
    mode: "double",
    difficulty: "daily",
    targetLength: 2,
    entries: ITEMS,
    ...config,
  };
  return render(
    <TrainerIndexProvider index={fixtureIndex()}>
      <PracticeView
        config={fullConfig}
        onExit={noop}
        onRestart={noop}
        onPracticeEntries={noop}
        onExitToToday={noop}
      />
    </TrainerIndexProvider>,
  );
}

function input(): HTMLElement {
  return screen.getByLabelText("编码输入区");
}

function press(key: string) {
  fireEvent.keyDown(input(), { key });
}

describe("PracticeSetupView", () => {
  beforeEach(() => {
    localStorage.clear();
    resetTrainerStore();
  });

  it("从设置页开始一场练习", async () => {
    const user = userEvent.setup();
    render(
      <TrainerIndexProvider index={fixtureIndex()}>
        <PracticeSetupView
          presetMode={null}
          reviewEntries={null}
          onPresetConsumed={noop}
          onExitToToday={noop}
        />
      </TrainerIndexProvider>,
    );
    await user.click(screen.getByRole("button", { name: /开始练习/ }));
    expect(await screen.findByLabelText("编码输入区")).toBeInTheDocument();
    expect(screen.getByText("行")).toBeInTheDocument();
  });

  it("模式选择覆盖全部 12 种模式并按分组展示", () => {
    render(
      <TrainerIndexProvider index={fixtureIndex()}>
        <PracticeSetupView
          presetMode={null}
          reviewEntries={null}
          onPresetConsumed={noop}
          onExitToToday={noop}
        />
      </TrainerIndexProvider>,
    );
    for (const label of [
      "双拼",
      "音形",
      "全码",
      "单字综合",
      "一级简码",
      "二码词简码",
      "零冲突词简码",
      "固定首码词简码",
      "固定词",
      "组句",
      "简码综合",
      "全模式综合",
    ]) {
      expect(screen.getAllByText(label).length).toBeGreaterThan(0);
    }
    for (const group of ["单字", "词语简码", "固定词", "组句", "综合"]) {
      expect(screen.getByRole("region", { name: group })).toBeInTheDocument();
    }
  });
});

describe("PracticeView", () => {
  beforeEach(() => {
    localStorage.clear();
    resetTrainerStore();
  });

  it("正确的键推进码位格", () => {
    renderPractice();
    press("x");
    expect(
      screen.getByLabelText("编码 2 键,已输入 1 键"),
    ).toBeInTheDocument();
  });

  it("错键不推进码位", () => {
    renderPractice();
    press("z");
    expect(
      screen.getByLabelText("编码 2 键,已输入 0 键"),
    ).toBeInTheDocument();
  });

  it("Backspace 删除最后一个已接受的键", () => {
    renderPractice();
    press("x");
    expect(
      screen.getByLabelText("编码 2 键,已输入 1 键"),
    ).toBeInTheDocument();
    press("Backspace");
    expect(
      screen.getByLabelText("编码 2 键,已输入 0 键"),
    ).toBeInTheDocument();
  });

  it("完整输入正确编码后自动进入下一题", async () => {
    renderPractice();
    press("x");
    press("k");
    expect(await screen.findByText("1 / 2")).toBeInTheDocument();
    // 反馈后自动前进,码位格归零
    await waitFor(() =>
      expect(
        screen.getByLabelText("编码 2 键,已输入 0 键"),
      ).toBeInTheDocument(),
    );
  });

  it("达到目标题数后进入小结", async () => {
    renderPractice();
    press("x");
    press("k");
    await waitFor(() =>
      expect(
        screen.getByLabelText("编码 2 键,已输入 0 键"),
      ).toBeInTheDocument(),
    );
    press("x");
    press("k");
    expect(await screen.findByText("本次练习")).toBeInTheDocument();
    expect(screen.getByText("再来一组")).toBeInTheDocument();
  });

  it("Escape 暂停,继续练习恢复", async () => {
    const user = userEvent.setup();
    renderPractice();
    press("Escape");
    expect(await screen.findByText("已暂停")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "继续练习" }));
    await waitFor(() =>
      expect(screen.queryByText("已暂停")).not.toBeInTheDocument(),
    );
    // 恢复后仍可输入
    press("x");
    expect(
      screen.getByLabelText("编码 2 键,已输入 1 键"),
    ).toBeInTheDocument();
  });

  it("暂停中可通过结束本次进入小结", async () => {
    const user = userEvent.setup();
    renderPractice();
    press("Escape");
    await user.click(
      await screen.findByRole("button", { name: "结束本次" }),
    );
    expect(await screen.findByText("本次练习")).toBeInTheDocument();
  });

  it("提示方式:始终显示时直接显示编码", () => {
    useTrainerStore.setState({ hintMode: "always" });
    renderPractice();
    expect(screen.getByText("xk")).toBeInTheDocument();
  });

  it("提示方式:错误后显示,按错前隐藏、按错后显示", () => {
    useTrainerStore.setState({ hintMode: "on-error" });
    renderPractice();
    expect(screen.queryByText("xk")).not.toBeInTheDocument();
    press("z");
    expect(screen.getByText("xk")).toBeInTheDocument();
  });

  it("提示方式:隐藏时不显示编码", () => {
    useTrainerStore.setState({ hintMode: "hidden" });
    renderPractice();
    expect(screen.queryByText("xk")).not.toBeInTheDocument();
    press("z");
    expect(screen.queryByText("xk")).not.toBeInTheDocument();
  });

  it("Ctrl 组合键不被拦截", () => {
    renderPractice();
    fireEvent.keyDown(input(), { key: "c", ctrlKey: true });
    expect(
      screen.getByLabelText("编码 2 键,已输入 0 键"),
    ).toBeInTheDocument();
  });

  it("答错后该题计入错题进度", async () => {
    renderPractice();
    press("z");
    press("x");
    press("k");
    await waitFor(() =>
      expect(
        useTrainerStore.getState().progress["行:xk"],
      ).toMatchObject({ attempts: 1, wrong: 1 }),
    );
  });
});
