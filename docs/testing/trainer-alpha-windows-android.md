# XHUP Flow Trainer Alpha 人工验收协议(Windows + Android)

本文档是 Trainer Alpha 阶段的**唯一人工验收标准**。目标平台只有:

- Windows 11 x64(NSIS setup.exe / MSI)
- Android 手机(signed universal APK,primarily arm64)

Linux、macOS、iOS 不在本阶段验收范围内,缺席不构成失败。

## 发布与测试的关系

GitHub Actions 构建成功只代表 **READY FOR HUMAN ALPHA TEST**,不代表
验收通过。每个 Alpha 预发布版本初始状态都是:

```text
Windows: UNVERIFIED
Android: UNVERIFIED
```

只有真实设备上的真人测试才能把状态改为 **PASS**。

## 结果状态与严重级

每条测试项标记四种状态之一:

| 状态 | 含义 |
| ---- | ---- |
| PASS | 表现符合预期 |
| FAIL | 表现不符合预期,必须记录严重级 |
| BLOCKED | 因环境/前置条件无法执行(需说明原因) |
| N/A | 该项在当前平台/版本不适用 |

失败按严重级分类:

| 级别 | 定义 |
| ---- | ---- |
| P0 | 无法安装/启动;数据损坏;核心训练完全不可用 |
| P1 | 核心训练功能错误;输入失效;数据持久化严重错误;主要页面无法使用 |
| P2 | 功能可继续使用,但布局/交互存在明显问题 |
| P3 | 轻微视觉/文案/间距问题 |

## Alpha 验收门槛

本阶段通过标准:

```text
P0 = 0
P1 = 0
Windows: PASS(所有适用项)
Android: PASS(所有适用项)
```

P2/P3 可以记录后留到下一个 Alpha 迭代修复。

## Alpha 迭代模型

```text
alpha.1 → 真人测试 → 记录失败
    ↓
代码修复 → alpha.2(新的 tag / 新构建)
    ↓
重测失败项 + 冒烟回归 → 依此迭代
```

**已发布的 Alpha 版本不可变**:每个测试过的二进制必须对应唯一的
tag 与 commit。发现问题不在原版本上覆盖修复,而是发布
`alpha.2`、`alpha.3`……

Android `versionCode` 与语义版本独立,**单调递增、永不回退**:

```text
alpha.1 → versionCode 1
alpha.2 → versionCode 2
alpha.3 → versionCode 3
```

这是 `alpha.1 → alpha.2` 覆盖安装更新(保留练习进度)的前提。

## 测试前准备

1. 从 GitHub Pre-release 下载对应平台资产与 `SHA256SUMS.txt`。
2. 校验完整性:
   - Windows(PowerShell):`Get-FileHash .\xhup-flow-trainer-v<版本>-windows-x64-setup.exe`
     与 `SHA256SUMS.txt` 中条目比对。
   - Android:`certutil -hashfile <apk> SHA256`(Windows)或
     `sha256sum <apk>`(macOS/Linux)比对。
3. 记录被测版本号(例如 `0.1.0-alpha.1`)与设备信息。

## 通用测试(C 系列)

两个平台都要执行。

| ID | 测试项 | 结果 |
| -- | ------ | ---- |
| C01 | 下载/安装包完整,校验通过 | |
| C02 | 首次启动成功 | |
| C03 | 无白屏 | |
| C04 | 训练数据(Trainer JSON)加载正常 | |
| C05 | 「今日」页面正常 | |
| C06 | 「练习」页面正常 | |
| C07 | 「错题」页面正常 | |
| C08 | 「键位」页面正常 | |
| C09 | 「设置」页面正常 | |
| C10 | 页面导航无明显卡顿 | |
| C11 | 双拼模式完成 20 题 | |
| C12 | 音形模式完成 20 题 | |
| C13 | 全码模式完成 20 题 | |
| C14 | 综合模式完成 30 题 | |
| C15 | 按错键不会推进编码格 | |
| C16 | 答错的字在若干题后重新出现 | |
| C17 | Backspace 删除上一键 | |
| C18 | Escape/暂停功能正常(适用处) | |
| C19 | 会话小结数据正确 | |
| C20 | 重启应用后本地进度保留 | |
| C21 | 主题设置保留 | |
| C22 | 重置进度功能正常 | |
| C23 | 断网离线启动正常 | |
| C24 | 全程无网络依赖 | |
| C25 | 连续 100 题无明显变慢 | |
| C26 | 无重复/重叠文字 | |
| C27 | 无横向页面溢出 | |
| C28 | 编码格清晰可见 | |
| C29 | 键盘标签清晰可读 | |
| C30 | 视觉质量可接受,适合日常使用 | |

## Windows 测试(W 系列)

| ID | 测试项 | 结果 |
| -- | ------ | ---- |
| W01 | NSIS setup.exe 可启动 | |
| W02 | MSI 可启动 | |
| W03 | NSIS 安装成功 | |
| W04 | MSI 安装成功(与 NSIS 分开测,必要时先卸载) | |
| W05 | 开始菜单条目存在 | |
| W06 | 应用图标/名称正确 | |
| W07 | 首次启动正常 | |
| W08 | 默认窗口尺寸合理 | |
| W09 | 最小窗口尺寸不破坏布局 | |
| W10 | 最大化正常 | |
| W11 | 窗口缩放正常 | |
| W12 | 100% DPI 显示正常 | |
| W13 | 125% DPI 显示正常 | |
| W14 | 150% DPI 显示正常 | |
| W15 | 深色模式正常 | |
| W16 | 浅色模式正常 | |
| W17 | 中文字体显示正常 | |
| W18 | 物理键盘 A–Z 输入正常 | |
| W19 | 快速击键不丢键 | |
| W20 | Backspace 正常 | |
| W21 | Escape 暂停正常 | |
| W22 | Ctrl+C 未被错误吞掉 | |
| W23 | Ctrl+L 未被错误吞掉(适用处) | |
| W24 | Ctrl+R 未被错误吞掉(适用处) | |
| W25 | Alt 组合键未被错误吞掉 | |
| W26 | 最小化/恢复正常 | |
| W27 | 切换其他应用后返回正常 | |
| W28 | 重启应用进度保留 | |
| W29 | 卸载成功 | |
| W30 | SmartScreen 行为已记录(出现警告属预期,不自动判 FAIL) | |
| W31 | 无明显视觉畸变 | |
| W32 | 30 分钟练习稳定 | |
| W33 | 500 题压力测试可接受 | |

## Android 测试(A 系列)

| ID | 测试项 | 结果 |
| -- | ------ | ---- |
| A01 | APK 可安装 | |
| A02 | 应用可启动 | |
| A03 | 应用名称/图标正常 | |
| A04 | 无意外危险权限请求 | |
| A05 | 竖屏 360–390px 布局正常 | |
| A06 | 横屏可用 | |
| A07 | 无横向溢出 | |
| A08 | 底部导航不被遮挡 | |
| A09 | Android 系统导航条不遮挡控件 | |
| A10 | 刘海/挖孔安全区可接受 | |
| A11 | Gboard 可输入练习内容 | |
| A12 | 小企鹅/Fcitx5 Android 可输入练习内容 | |
| A13 | 首个输入字符不丢失 | |
| A14 | 快速输入不丢键 | |
| A15 | 点按练习区域可重新唤起软键盘 | |
| A16 | 屏幕键盘可完成整题 | |
| A17 | 键盘按钮易于点按(≥44px) | |
| A18 | 软键盘不遮挡目标汉字 | |
| A19 | 软键盘不遮挡编码格 | |
| A20 | 答对自动推进 | |
| A21 | 按错键反馈可见 | |
| A22 | 错题正确回炉 | |
| A23 | Backspace 行为正常 | |
| A24 | 应用切后台再回前台可恢复 | |
| A25 | 切换应用再返回正常 | |
| A26 | 锁屏再解锁正常 | |
| A27 | 旋转设备布局仍可用 | |
| A28 | 深色模式正常 | |
| A29 | 浅色模式正常 | |
| A30 | 重启应用进度保留 | |
| A31 | 同签名/versionCode 递增的 Alpha 覆盖更新后进度保留 | |
| A32 | 离线可用 | |
| A33 | 100 题练习稳定 | |
| A34 | 30 分钟练习稳定 | |
| A35 | 无明显发热 | |
| A36 | 无异常耗电 | |
| A37 | 无文字重叠 | |
| A38 | 无按钮超出可视区 | |
| A39 | 键盘标签清晰可读 | |
| A40 | 整体可作为真实手机应用日常使用 | |

## 结果汇报格式

全部通过时,无需逐项粘贴,简报到版本即可:

```text
alpha.1 人工验收完成

Windows 11 x64: PASS
Android <设备型号/系统版本>: PASS

P0: 0  P1: 0  P2: 0  P3: 0

核心训练正常 / 视觉布局正常 / 输入正常 / 持久化正常 / 离线正常
```

存在失败时,只汇报失败项 + 上下文:

```text
A18 FAIL / P1
软键盘弹出后编码区域完全被遮挡
设备: Pixel 8 / Android 15 / 竖屏
可复现: 100%
截图: [附图]
```

视觉类失败报告字段:

```text
Test ID / Platform / Device / OS 版本 / 屏幕尺寸或 DPI(如已知)
预期表现 / 实际表现 / 是否可复现 / 截图 / 严重级
```

## 安装指引

### Windows(NSIS / MSI)

Alpha 安装包**未进行 Authenticode 代码签名**。Windows SmartScreen
可能显示「Windows 已保护你的电脑」或「未知发布者」——对 Alpha 而言
这是**预期行为**,不自动判 FAIL。请仅对来自本仓库 GitHub
Pre-release、且 SHA-256 校验一致的安装包选择继续运行。不要为此全局
关闭 SmartScreen 或系统安全防护。

安装包损坏、无法继续、安装后应用无法运行 → 均为 FAIL(通常 P0/P1)。

### Android(侧载 APK)

1. 从 GitHub Pre-release 下载 universal APK 到手机。
2. Android 可能要求允许「浏览器/文件管理器」这一来源安装应用,
   只给当前使用的来源授权,不要全局放开未知来源。
3. 安装并启动。测试结束后可撤销该来源的安装授权。

## Android 签名密钥管理(一次性配置)

Alpha APK 使用项目固定的 Android 签名密钥签名。密钥即应用身份:

- **只生成一次**,离线安全备份;丢失密钥 = 未来所有覆盖更新断裂。
- **永不提交到版本库**;CI 只消费 GitHub Secrets 中的副本。
- 不要在文档、Issue、聊天记录中粘贴密码。

生成密钥(在本机安全环境执行一次):

```bash
keytool -genkeypair \
  -keystore ~/xhup-flow-android.jks \
  -storetype JKS \
  -keyalg RSA \
  -keysize 2048 \
  -validity 10000 \
  -alias xhup-flow
```

写入 GitHub Secrets(交互式/stdin 读入,避免密码进入 shell 历史):

```bash
base64 -w0 ~/xhup-flow-android.jks | gh secret set ANDROID_KEY_BASE64
gh secret set ANDROID_KEY_ALIAS        # 例如 xhup-flow
gh secret set ANDROID_KEY_PASSWORD     # 交互输入
gh secret set ANDROID_STORE_PASSWORD   # 交互输入
```

所需 Secret 名称(仓库 Settings → Secrets and variables → Actions):

```text
ANDROID_KEY_BASE64
ANDROID_KEY_ALIAS
ANDROID_KEY_PASSWORD
ANDROID_STORE_PASSWORD
```

## 触发一次真实 Alpha 发布(仅维护者,且只能从合并后的 main)

```bash
gh workflow run trainer-alpha.yml \
  --ref main \
  -f version=0.1.0-alpha.1 \
  -f android_version_code=1 \
  -f publish=true

gh run watch
```

`-f publish=false` 为演练模式:完整构建、签名(若 Secret 已配置)、
校验、上传工作流产物,但**不创建 GitHub Release**。

已存在的 `trainer-v<版本>` tag 或同名 Release 会导致发布失败——
Alpha 版本不可变,修复请递增为 `alpha.2` 并递增 `android_version_code`。
