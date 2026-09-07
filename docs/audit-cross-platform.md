# 跨平台最终对抗性审计(里程碑 #46)

审计对象:#41–#45 叠加栈(共享核心 / 桌面训练器 / 微信小程序 / Rime 部署)。
结论:无阻断问题;全部安全不变量成立。

## 1. 平台泄漏(共享核心)

- 机器可执行守卫:`packages/trainer-core/src/purity.test.ts`
  (禁 window/document/localStorage/navigator/react/@tauri-apps/wx./Taro/import.meta/裸 fetch)。
- 独立 grep 复核(排除测试与注释):**零命中**。
- 核心测试 node 环境运行:任何 DOM 依赖进核心即失败(144 用例)。

## 2. Rust panic / 错误审计

- `trainer/src-tauri/src/manager.rs`、`commands.rs` 生产路径:**0 处**
  `unwrap()/expect()/panic!`(83 处全部位于测试模块)。
- 管理错误全部为类型化 `ManagerError`(缺失目录/包无效/IO/回滚),
  计划执行有暂存→提交→回滚事务与信任边界校验(只写 OWNED_FILES)。

## 3. 数据迁移与损坏容错

- 桌面 store:persisted v1→v2 逐字段迁移 + 逐字段校验降级(测试覆盖);
  `lessonEvidence` 以可选字段加入,v2 版本号不变,旧数据缺字段得 `{}`。
- 小程序:`state-schema.ts` 逐字段校验降级,v1→v2 迁移;损坏存储回退
  空进度并留证据日志,绝不整体清空(测试覆盖往返/损坏/未知版本)。
- 备份:BACKUP_VERSION 2,导出确定性;旧备份导入新字段得默认值,
  新备份对旧导入方优雅忽略(核心备份测试覆盖)。

## 4. 安全

- 密钥扫描:仓库内无 AppSecret / 上传私钥 / 口令;小程序 project.config
  仅 `touristappid` 测试号占位;代理配置仅存 git local 配置,不入库。
- 无 shell 插值:部署/重部署全部结构化进程调用;路径信任边界校验
  (绝对路径/`..`/非拥有文件拒绝,符号链接拒绝)。
- 小程序:无登录/无云/无遥测;进度仅本地。
- 真机探针只读(绝不提交、不写学习数据);学习数据测试全部在隔离临时目录。

## 5. 统计正确性重点场景

- 暂停/后台:引擎 `pause/resume` 结清 activeMs;桌面 `visibilitychange`
  与小程序 `useDidHide` 均不计后台时长(测试覆盖)。
- 中止/重试/跨日:既有 store/统计测试覆盖(localDateKey 按本地日历日)。

## 6. 冻结语义回归

- librime 全量运行时审计(静态等值 / FIXED_FIRST A/B / 二码全量 /
  学习持久化 / 日常输入控制)CI 12/12 绿。
- 冻结映射改动:**0**;静态候选回归:**0**。

## 7. 无障碍与平台语义

- 桌面:既有 focus-visible / 键盘导航 / 非纯色反馈维持(本轮未发现回归)。
- 小程序:按平台能力使用原生导航/tabBar/震动语义映射(off/light/medium
  → 无/light/medium/heavy,不假装档位差异)。

## 遗留(如实记录,非阻断)

- 逐次形码混淆对未持久化(键位级代理);形码笔画数据按来源政策未引入。
- 小程序暂无按日趋势统计页与备份导入 UI。
- WeChat 硬件返回无法拦截(平台限制)。
