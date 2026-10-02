# Windows MSI 许可资源冲突：根因与验证

未签名演练 `37035751260` 的应用与 NSIS 编译成功，MSI 在 WiX
`light.exe` 失败。详细演练 `37038701415` 的生成 `main.wxs` 和日志
确认 `LGHT0204 / ICE30`：多个 `LICENSE` / `COPYRIGHT` 文件被不同
component 安装到同一个 `licenses/` 目录。

原因不是缺少数据，也不是需要关闭 ICE 验证：当前 Tauri WiX 生成的
`File` 元素没有 `Name`，安装名取源文件 basename。配置中改名为
`glib-MIT.txt`、`urlpattern-MIT.txt` 的不同 destination 在 Linux
和 NSIS 上可用，但不能防止 MSI 中重复的原始 `LICENSE`。

修复将每一来源放在 `licenses/<来源>/`，并保留源 basename。
不修改 vendored 上游许可内容，不复制可能漂移的手工副本，不 suppress ICE30。
新增配置回归拒绝依赖文件名重映射与大小写不敏感的路径碰撞。

验证分两层：
1. 原始源码/许可哈希门禁、资源清单和碰撞负向测试；
2. 实际产品流水线构建 MSI 后执行私有临时目录中的 `msiexec /a`
   管理提取，再逐文件比较全部许可资源与源码字节。不是用户安装，
   不启动 Trainer，也不访问真实 Rime 用户目录。
   Linux 继续从实际 deb 提取后运行相同的字节检查。

详细 WiX 日志/失败输入只作诊断资产，不能算成功安装包。Windows/macOS
手工安装、输入法交互、升级及卸载验收仍由用户按 README 执行；
自动构建成功不替代这些验收。

同时修正两个只在 Windows 分支出现的 Rust `unused_mut`，
并把 Trainer/CLI 的 `clippy -D warnings` 加入三平台原生 CI。
未关闭警告，也没有绕过分支或发布门禁。
