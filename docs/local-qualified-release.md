# 本地完整运行时证据与发布边界

本工具保留全量本地测试、16 分片和独立重新收集能力。
当前 `local-runtime-evidence-v2`（报告 schema 2）只证明实际执行的本地检查；
`platform_acceptance` 固定为四个平台 UNVERIFIED，不是正式发布授权。
正式版另须满足[完整平台与同源云端资格](runtime-qualified-release.md)，
不能用本地报告自动继承 v2.0.0 的历史豁免。

CI / Full Regression 仍保留 `workflow_dispatch` 入口；是否自动触发与本次是否
已实际执行是两回事。取消、失败或未运行的测试不能记作 PASS。

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
  --artifacts /absolute/downloaded-versioned-assets --artifact-version 2.1.0-rc.1 \
  > LOCAL-QUALIFICATION.json
```

它要求当前 BuildManifest schema 2、同源同版本、完整 11 项封存负载
（8 个产品包与 3 个元数据文件）及精确字节哈希。也支持独立构建的正式版本，
不把 RC 文件名或内嵌版本当成正式版本。LOCAL-VALIDATION.json 与完整原始证据
须保留；不能将本地执行说成云端测试成功，更不能据此宣称平台已验收。

## 3. 发布仍执行独立门禁

本地报告不创建 Release，也不允许直接绕过正式发布工作流。
RC 可在核验当前构建后以明确的 UNVERIFIED 状态发布；
正式版须对正式版本的真实字节重新验收，并提交 schema 3 平台收据、
同源 CI/全量原生/独立打包记录。发布步骤不能再次重建受验安装包。

安装、备份与恢复按绑定源码的安装文档执行，不删除用户词典或无关配置。
当前平台、签名或权限不足必须明确记录；没有证据就不生成 PASS。
