> 历史设计记录：当前 Trainer 已移除输入法管理及内嵌安装包，本文描述的安装/导出功能不再提供。当前安装方式见[手动教程](install-guide.zh-CN.md)。

# Trainer 平台中立包导出

Linux 实际 RC.3 `.deb`（演练 run 37041603245）在独立 HOME/XDG 目录、
Xvfb 与 WebKitWebDriver 下启动成功；界面安装计划和确认会写入嵌套 Lua，
并保留无关用户配置。真实 WebKit 键盘输入也能完成一题并写入本地练习进度。
这些是自动化桌面行为，不是 fcitx5 用户会话或 Android 真机验收。

原 `product_export_package` 对嵌套 `lua/xhup_flow/*` 直接写文件，未建立
父目录，实际 IPC 返回 `No such file or directory`，还缺少数据许可说明。
“二进制构建成功”不能代替这个导出功能的验证。

修复由 `package_export.rs` 实现：

- 只取构建期只读包，不复制真实用户目录或 userdb；严格核对 23 个生成文件（含独占方案列表 default.custom.yaml）。
- 目标父目录须已存在、不是符号链接；版本不能构造路径逃逸。
- 排他创建版本目录；已有目录、文件和符号链接一律拒绝，不静默覆盖旧导出。
- 建立固定子目录，排他写文件并同步；输出 23 个生成文件和 8 个安装/来源/许可文件，
  与发布 ZIP 的逻辑内容一致。
- 写入失败时清理本次新建目录，清理失败也报告；不会把残缺输出报告为成功。

**限制**：这不是目录级断电事务；进程被杀死可能留下残缺目录。重试会拒绝它，
用户应检查后移走该残缺输出，或选另一个父目录。不宣称防御同用户恶意进程
同时替换父目录，也不证明 Android 客户端已经部署该包。

七个 Rust 回归覆盖实际随附包全文件字节、嵌套路径、许可证、无用户文件，
错误清理/重试、已存在输出、路径逃逸、符号链接和并发只有一个成功者。
实际修复后桌面回归另外记录二进制 SHA256 与 source/run，不能继承上述
失败演练的成功状态。

## 真实桌面回归

`tests/desktop/linux_artifact_smoke.py` 使用独立 HOME/XDG、Xvfb 和
WebKitWebDriver，调用真实前端及 Rust IPC，不替换产品 API。检查安装确认前
不写入、完整导出与许可字节、拒绝覆盖后字节不变、键盘练习、暂停/继续，以及
实际关闭再启动后的本地进度。默认短暂答题反馈阶段不能暂停，因此测试等待
下一题后再验证暂停，不通过任意延迟或重复点击掩盖失败。

修复提交 `2937ba5bbedcfdf2ddd5a0e9b35a9afe81526607` 的本地
`tauri/custom-protocol` 二进制已通过 17 项检查，SHA256 为
`0af63dfc8c2480e8adb112d3503a97dcd9b7bf5141d28501fc5a0805193b119e`。
该构建仍为 RC.2 源码版本，是修复集成证据，不是已发布 RC.3 的证明。

Linux 打包工作流在实际 `.deb` 提取后运行相同门禁，记录版本、源码提交、
二进制/编码表哈希和检查结果；失败日志也单独上传，不混入正式安装包清单。
最终产物必须重新通过，不能继承本地结果。Windows/macOS 人工验收仍按 README。

```sh
cargo run --locked -q -p xhup-core --example syllable_codes > /tmp/syllables.tsv
dbus-run-session -- python3 tests/desktop/linux_artifact_smoke.py \
  --binary /absolute/path/to/extracted/usr/bin/trainer --source-root "$PWD" \
  --syllables /tmp/syllables.tsv --version 2.0.0-rc.3 --report /tmp/desktop-report.json
```
