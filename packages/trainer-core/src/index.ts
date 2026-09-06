/**
 * @xhup/trainer-core —— XHUP Flow 跨平台训练/学习核心。
 *
 * 平台中立:不依赖 window / document / localStorage / navigator /
 * React / Tauri / wx.*。宿主平台差异通过 src/adapters 的契约注入。
 * 规范数据一律来自 Rust 生成产物;本包只承载训练与学习语义。
 */

// 数据契约与校验
export * from "./data/trainer-data";
export * from "./data/trainer-index";

// 练习域
export * from "./practice/types";
export * from "./practice/engine";
export * from "./practice/scheduler";

// 学习域
export * from "./learning/progress";
export * from "./learning/daily-stats";
export * from "./learning/weakness";
export * from "./learning/review";
export * from "./learning/stats";
export * from "./learning/backup";

// 学习路径与自适应掌握(里程碑 44)
export * from "./learning-path/lesson-state";
export * from "./learning-path/recommendation";
export * from "./learning-path/shape-mastery";
export * from "./learning-path/checkpoints";

// 课程内容
export * from "./lessons/content";
export * from "./lessons/shape-explorer";

// 宿主能力契约
export * from "./adapters";

// 国际化(纯 TS 翻译表,宿主只负责接线)
export * from "./i18n";
