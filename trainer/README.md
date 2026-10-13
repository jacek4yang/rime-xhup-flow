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

前端加载时完整校验 `schemaVersion: 4` 契约(条目 + 词语 +
canonical v2 PRIMARY/FIXED_FIRST 简码 + 组句 fixtures + 双拼映射;
字段、码长一致性、core/extended scope、编码来源/provenance、`(char, code)`
唯一性、`frequencyScore` 安全整数等),校验失败会显示可读错误而不是
白屏。本地 V1/V2 进度原位迁入 V3,保留旧存储键和历史统计;V2 备份仍可导入。

参考页与生产生成器共享同一 `InputHanzi` 数据：搜索「嗯」「诶」会显示
规范 core / 扩展输入、官网/兼容来源及 alternate code；搜索多字文本（如
「提示词」）会按相同两键音码原语展示可输入组合码。

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

## 纯训练边界

Trainer 在桌面、Web 和 Android 上只负责练习、学习教程、错题、统计、
键位参考与训练进度备份。无需 Rime 即可启动和练习。
Tauri 只承载本地前端，不注册应用自定义 IPC，不链接输入法安装/学习库管理核心，
不内嵌 Rime 安装包，也不读取或改写输入法用户目录。

输入法安装由用户按[手动教程](../docs/install-guide.zh-CN.md)完成。
`xhup-flow.trainer.v2` 的训练进度与 Rime 的 `xhup_flow_user.userdb` 是独立数据。

## 练习模式

| 模式 | 码长 | 内容 |
| ---- | ---- | ---- |
| 双拼 | 2 键 | 小鹤双拼音码 |
| 音形 | 3 键 | 双拼 + 首形 |
| 全码 | 4 键 | 完整全码,核心肌肉记忆 |
| 综合 | 2/3/4 | 三种码长均衡轮换 |

难度按规范词频分池:入门(前 800)/ 日常(前 3000)/ 完整(全部)。
答错的题会在 3–8 题后自然回炉;掌握度与按日统计保存在本机。

### 跨天复习与迁移

十二种常规模式继续可用。每题实际完成后,共享核心记录主练/备用路线、
答案提示、错键、活跃用时和独立回忆证据。主练码零错且未看答案才可晋级;
备用全码只是合法兜底,不算记住简码。

复习间隔为 1/3/7/14/30/90 个 24 小时周期。同一到期日之前重复完成不刷等级,
也不推迟到期时间。提示、错键或备用路线安排 10 分钟后再练;每字超过 8 秒的
独立完成保留回忆证据,但不晋级,最迟次日复习。这是确定性训练规则,不是经
用户实验校准的遗忘曲线。常规题池内到期优先,仍保留最近题排除和会话内回炉。

复习入口同时列出到期条目与错题,支持一级简码、长词和句子,不再只轮换
2/3/4 键条目。旧记录保留 mastery、次数和用时,但复习证据置为未知,需要一次
新的独立回忆,不会把旧 mastery 捏造成跨天掌握。

新导出备份为 V3,包含版本化复习状态;桌面与小程序共享校验和更新算法。
旧应用不能导入 V3 备份。降级前保留新备份,旧版重新保存本地状态可能丢弃
它不认识的复习字段,但不应把未知状态推定为已掌握。

验证包括真实 Chromium 页面上的 V2 恢复、一级简码提交、重载及受控跨日时钟;
不代表 Tauri 壳或微信真机验收。

## 本地隐私

练习记录仅保存在本机浏览器 / 应用本地存储中(`localStorage`),
没有任何统计上报、账号或云同步。

## Alpha 测试

Trainer 的 Windows x64 与 Android Alpha 预发布版通过 GitHub
Pre-release 分发(手动触发,tag 形如 `trainer-v1.0.0-alpha.1`(core 随产品版本))。
真人验收清单、结果汇报格式与 Android 签名密钥管理见
[docs/testing/trainer-alpha-windows-android.md](../docs/testing/trainer-alpha-windows-android.md)。
