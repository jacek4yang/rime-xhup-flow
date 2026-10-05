# 本地完整验收，云端仅最终打包发布

仓库所有者明确选择本流程，避免在 GitHub Actions 重复执行完整测试。
CI / Full Regression 保留 `workflow_dispatch` 诊断入口，不再自动触发。
取消或未运行的云端测试不是 PASS，不改变分支保护，也不伪造 CI 运行证明。
旧完整平台门禁和旧云端 runtime-qualified 流程保留，但本流程不冒用它们的资格。

## 1. 冻结源码并完成本地验收

合并经本地测试的修改后固定完整源 SHA；工作树必须干净，运行期间不可修改。
在工具齐备的 Linux 主机执行（目标目录与证据目录须位于仓库之外，证据目录必须新建）：

```sh
python3 tests/release/local_qualification.py run \
  --out /absolute/new-evidence \
  --target /absolute/separate-cargo-target \
  --plugin /absolute/supported-librime-lua.so --jobs 8
```

包括 Rust 完整 workspace/all-targets、格式、Clippy、维护依赖 `-D warnings`、
安全/维护源完整性与 RustSec 审计；前端冻结安装、类型检查、测试、Trainer 和
小程序构建、完整原始依赖审计；Python 回归；真实 librime 部署/独占/控制/Lua/
上下文测试；同一完整清单的全部 16 分片和收集器。每个分片均保留学习、重启、
写入者等原有验证，不能用采样替代。平台图形前端人工测试仍未完成。

证据保留命令、退出码、日志 SHA256、完整输入/输出清单与原始收据。失败仍写
FAIL 报告，不能发布。`verify` 重新汇总全部分片并比较覆盖结果，不只相信摘要：

```sh
python3 tests/release/local_qualification.py verify \
  --out /absolute/new-evidence --source FULL_SOURCE_SHA
```

这是仓库所有者控制主机的验收记录，不是防御主机失陷的密码学证明。
合成单元测试只检验证据流程，不是产品验收。

## 2. 云端只构建最终产品

本地全部通过后，对同一 main 源 SHA 调用现有 RC Release 工作流，
`publish=true`、版本为当前 RC 版本。它完成跨平台构建、既有 Android 密钥签名、
安装包检查、Linux 实际产物 GUI 检查和封装来源，先创建草稿。
这些是最终产品检查，不是重新执行完整 Rust/前端测试和全量 librime 分片。
不得替换 Android 密钥；Windows/macOS 未签名，macOS 未公证，必须公开说明。

独立查询该运行成功状态、实际 head_sha 和草稿目标源，下载真实全部附件；
不能用本地重建包替代下载包。然后执行：

```sh
python3 tests/release/local_qualification.py verify-build \
  --out /absolute/new-evidence --source FULL_SOURCE_SHA \
  --artifacts /absolute/downloaded-rc-assets --artifact-version 2.0.0-rc.3 \
  > LOCAL-QUALIFICATION.json
```

它要求同源 BUILD-MANIFEST、完整 11 项已封装负载（8 个产品包与 3 个元数据文件）
及精确字节哈希，拒绝未知/缺失/改变的附件。保留 LOCAL-VALIDATION.json 和完整
可重新收集的证据归档，公开记录实际云端构建运行 URL；不要制造云端测试成功证明。

## 3. 所有者按此明确范围发布

由所有者通过 GitHub Release API 创建 v2.0.0 草稿，复用上述已核验的 RC 原字节，
附加 LOCAL-QUALIFICATION.json、LOCAL-VALIDATION.json 及完整证据归档。
发布说明必须标注本地验收模式、确切源码/构建运行、平台 UNVERIFIED 状态、
签名限制，以及附件名称和内嵌版本仍为 RC（不偷偷重编）。
再次核对草稿实际附件后公开，再下载公开附件核验相同哈希。
此直接发布流程基于所有者明确授权，不声称通过旧云端 CI/full-platform 晋升门禁。

Rime 内独占、首次原配置备份和恢复按安装文档执行，不删除系统输入法。
发布不等于全平台人工验收通过，先在目标设备小范围使用并保留恢复路径。
