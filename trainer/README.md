# XHUP Flow · 小鹤音形训练

小鹤音形(XHUP)打字训练器:从双拼到全码的汉字编码肌肉记忆练习。
桌面应用(Tauri)与响应式 Web 共用同一份前端。

## 技术栈

- React 19 + TypeScript + Vite
- Tailwind CSS v4 + shadcn/ui(本地组件)+ lucide-react
- zustand(持久化本地进度)+ motion(克制的动效)
- Vitest + Testing Library
- Tauri 2(桌面壳,薄平台边界)

## 数据来源架构

**前端不维护任何码表。** 双拼映射、汉字编码、词频与 Rime 权重全部由
Rust 生成器统一产出:

```text
xhup-cli generate trainer  →  trainer/public/generated/xhup_flow_trainer.json
```

`pnpm dev` 与 `pnpm build` 都会先执行 `generate:data` 重新生成规范
JSON;生成失败则前端命令直接失败,不会回退到过期数据。生成的 JSON
属于构建产物,已被 `.gitignore` 忽略,不进入版本库。

前端加载时完整校验 `schemaVersion: 1` 契约(字段、码长一致性、
`(char, code)` 唯一性、`frequencyScore` 安全整数等),校验失败会显示
可读错误而不是白屏。

## 开发命令

```bash
pnpm install          # 安装依赖
pnpm dev              # 生成训练数据 + 启动 Vite 开发服务器
pnpm typecheck        # tsc --noEmit
pnpm test             # Vitest 单元/组件测试
pnpm test:watch       # 监听模式
pnpm build            # 生成训练数据 + 类型检查 + 生产构建
pnpm preview          # 预览生产构建
pnpm tauri dev        # Tauri 桌面开发
pnpm tauri build      # Tauri 桌面打包
```

## 练习模式

| 模式 | 码长 | 内容 |
| ---- | ---- | ---- |
| 双拼 | 2 键 | 小鹤双拼音码 |
| 音形 | 3 键 | 双拼 + 首形 |
| 全码 | 4 键 | 完整全码,核心肌肉记忆 |
| 综合 | 2/3/4 | 三种码长均衡轮换 |

难度按规范词频分池:入门(前 800)/ 日常(前 3000)/ 完整(全部)。
答错的题会在 3–8 题后自然回炉;掌握度与按日统计保存在本机。

## 本地隐私

练习记录仅保存在本机浏览器 / 应用本地存储中(`localStorage`),
没有任何统计上报、账号或云同步。
