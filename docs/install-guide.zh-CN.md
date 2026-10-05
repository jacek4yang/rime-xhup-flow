# XHUP Flow v2.0.0 手动安装指南

> 本指南对应 v2.0.0 发布附件；正式发布前下载地址可能尚不可用。
> 请以 Release 中实际附件、SHA-256 和验收范围为准。

## 1. 先选需要安装的部分

- **只想在各个应用里打字**：安装本系统的 Rime 输入法前端，再手动安装 Rime 源包。
- **想练习键位、编码和输入**：另装可选的 XHUP Flow Trainer。
- **Android 要实现两者**：安装 Trainer APK；另外安装 Fcitx5 for Android 主程序和 RIME 插件，再手动放入方案。Trainer 是训练应用，不是 Android 系统键盘。

Trainer 不负责安装、升级、修复、卸载或部署 Rime 方案。训练记录和 Rime 的学习词库是两份独立数据。

下载入口：[v2.0.0 Release](https://github.com/jacek4yang/rime-xhup-flow/releases/tag/xhup-flow-v2.0.0)。
在该页的 Assets 里选附件，不要把 GitHub 自动提供的 `Source code (zip)` 当作可直接部署的 Rime 包。

| 用途 | 附件文件名 |
| --- | --- |
| 所有平台的 Rime 方案 | `xhup-flow-rime-v2.0.0-rc.3.zip` |
| Android ARM64 Trainer | `xhup-flow-trainer-v2.0.0-rc.3-android-arm64.apk` |
| Android 多架构 Trainer | `xhup-flow-trainer-v2.0.0-rc.3-android-universal.apk` |
| Windows x64 Trainer，二选一 | `xhup-flow-trainer-v2.0.0-rc.3-windows-x64-setup.exe` 或 `xhup-flow-trainer-v2.0.0-rc.3-windows-x64.msi` |
| macOS 通用 Trainer | `xhup-flow-trainer-v2.0.0-rc.3-macos-universal.dmg` |
| Debian/Ubuntu x64 Trainer | `xhup-flow-trainer-v2.0.0-rc.3-linux-amd64.deb` |
| Fedora/RPM x64 Trainer | `xhup-flow-trainer-v2.0.0-rc.3-linux-x86_64.rpm` |

同时下载 `SHA256SUMS.txt`。若稳定版沿用 RC 附件字节，文件名及应用内部版本保留 RC 标记是预期行为；以本次 Release 的说明与哈希为准。

**验收边界**：底层运行时资格不代表用户平台已经通过实际输入法验收。Windows、macOS、Android 的人工安装和输入测试仍以该 Release 的 `ACCEPTANCE.json` 为准，`UNVERIFIED` 表示未验。Windows/macOS Trainer 未签名，macOS 未公证。遇到系统安全拦截时先核对来源、哈希和发布说明；本文不要求关闭 SmartScreen、Gatekeeper 或 Play Protect。

## 2. 在电脑下载并核对 SHA-256

下面先演示 Android 所需的 Trainer 和通用 Rime ZIP；桌面用户也使用同一下载函数，只需在后文选自己的附件。必须先校验成功，再安装或解压。哈希比对用于确认下载字节与发布清单一致，不等同于平台签名或人工验收。

### Windows：PowerShell

打开普通 PowerShell，按块复制。这里使用 `$HOME\Downloads\XHUP-v2.0.0`，不需要管理员权限。

```powershell
$ErrorActionPreference = 'Stop'
$AssetVersion = '2.0.0-rc.3'
$Base = 'https://github.com/jacek4yang/rime-xhup-flow/releases/download/xhup-flow-v2.0.0'
$Work = Join-Path $HOME 'Downloads\XHUP-v2.0.0'
New-Item -ItemType Directory -Force -Path $Work | Out-Null
Set-Location $Work
Invoke-WebRequest "$Base/SHA256SUMS.txt" -OutFile 'SHA256SUMS.txt'

function Get-XhupAsset([string]$Name) {
    Invoke-WebRequest "$Base/$Name" -OutFile $Name
    $Pattern = '^[0-9a-fA-F]{64}\s+\*?' + [regex]::Escape($Name) + '$'
    $Lines = @(Get-Content 'SHA256SUMS.txt' | Where-Object { $_ -match $Pattern })
    if ($Lines.Count -ne 1) { throw "哈希清单中没有唯一的文件记录：$Name" }
    $Expected = ($Lines[0] -split '\s+')[0]
    $Actual = (Get-FileHash -LiteralPath $Name -Algorithm SHA256).Hash
    if ($Actual -ine $Expected) { throw "SHA-256 不匹配，停止：$Name" }
    Write-Host "SHA-256 OK: $Name"
}

$Apk = "xhup-flow-trainer-v$AssetVersion-android-universal.apk"
$RimeZip = "xhup-flow-rime-v$AssetVersion.zip"
Get-XhupAsset $Apk
Get-XhupAsset $RimeZip
```

### macOS / Linux：终端

```sh
ASSET_VERSION='2.0.0-rc.3'
BASE='https://github.com/jacek4yang/rime-xhup-flow/releases/download/xhup-flow-v2.0.0'
mkdir -p "$HOME/Downloads/XHUP-v2.0.0"
cd "$HOME/Downloads/XHUP-v2.0.0" || exit
curl -fL --retry 3 "$BASE/SHA256SUMS.txt" -o SHA256SUMS.txt

download_xhup() (
    set -eu
    file="$1"
    curl -fL --retry 3 "$BASE/$file" -o "$file"
    expected=$(awk -v f="$file" '$2 == f {print $1}' SHA256SUMS.txt)
    [ "${#expected}" -eq 64 ] || { echo '缺少或重复的哈希记录'; exit 1; }
    if command -v sha256sum >/dev/null 2>&1; then
        actual=$(sha256sum "$file" | awk '{print $1}')
    else
        actual=$(shasum -a 256 "$file" | awk '{print $1}')
    fi
    [ "$actual" = "$expected" ] || { echo "SHA-256 不匹配，停止：$file"; exit 1; }
    printf 'SHA-256 OK: %s\n' "$file"
)

APK="xhup-flow-trainer-v$ASSET_VERSION-android-universal.apk"
RIME_ZIP="xhup-flow-rime-v$ASSET_VERSION.zip"
download_xhup "$APK" && download_xhup "$RIME_ZIP"
```

下载失败、404、哈希不一致或缺少清单记录，都应停止，不要跳过校验。Linux/macOS 示例中函数失败不会关闭交互终端；看到错误后不要继续执行后面的安装命令。

## 3. Android：电脑通过 ADB 安装 Trainer

### 3.1 准备 ADB 和手机

1. 从 Google 的 [SDK Platform Tools 官方页面](https://developer.android.com/tools/releases/platform-tools) 下载适合电脑系统的 ZIP，阅读并接受页面要求的条款。只做 ADB 安装不需要完整 Android Studio。
2. 解压到 `$HOME/Android`，使 Windows 上存在 `Android\platform-tools\adb.exe`，macOS/Linux 上存在 `Android/platform-tools/adb`。如果已装 Android SDK，也可将下方 ADB 变量改为现有 `platform-tools` 路径。
3. 手机打开开发者选项中的“USB 调试”，使用能传数据的 USB 线连接电脑。解锁手机，核对并允许这台可信电脑的 USB 调试/RSA 授权。授权必须在手机上完成。Windows 若缺驱动，使用设备厂商的官方 USB 驱动；Linux 权限问题按 Google 的[真机连接说明](https://developer.android.com/studio/run/device)处理。
4. 暂不打开无线调试；完成后可关闭 USB 调试并撤销不再需要的电脑授权。USB 调试会授予电脑较强的设备控制能力。

连接、设备选择和安装参数参考 [Google ADB 文档](https://developer.android.com/tools/adb)及 [ADB 官方手册](https://android.googlesource.com/platform/packages/modules/adb/+/refs/heads/main/docs/user/adb.1.md)。

### 3.2 Windows PowerShell

继续使用第 2 节的 PowerShell 窗口：

```powershell
$Adb = Join-Path $HOME 'Android\platform-tools\adb.exe'
& $Adb version
& $Adb devices -l
```

找到自己的手机，状态必须为 `device`。把下面的示例字符串替换为列表里的真实序列号，不要原样保留。连接多个手机或模拟器时，后续始终使用 `-s`。

```powershell
$Serial = '填入你的设备序列号'
& $Adb -s $Serial shell getprop ro.product.cpu.abilist
& $Adb -s $Serial shell getprop ro.build.version.release
& $Adb -s $Serial install -r (Join-Path $Work $Apk)
if ($LASTEXITCODE -ne 0) { throw 'APK 安装失败；请先处理下方对应错误' }
& $Adb -s $Serial shell pm path io.github.jacek4yang.xhupflow
& $Adb -s $Serial shell am start -n 'io.github.jacek4yang.xhupflow/.MainActivity'
```

### 3.3 macOS / Linux 终端

```sh
ADB="$HOME/Android/platform-tools/adb"
"$ADB" version
"$ADB" devices -l
```

将序列号替换为真实值后执行：

```sh
SERIAL='填入你的设备序列号'
"$ADB" -s "$SERIAL" shell getprop ro.product.cpu.abilist
"$ADB" -s "$SERIAL" shell getprop ro.build.version.release
"$ADB" -s "$SERIAL" install -r "$APK" &&
"$ADB" -s "$SERIAL" shell pm path io.github.jacek4yang.xhupflow &&
"$ADB" -s "$SERIAL" shell am start -n io.github.jacek4yang.xhupflow/.MainActivity
```

`Success` 表示 APK 安装完成；还需亲自打开应用检查练习界面。`-r` 用于覆盖安装并保留应用数据，但不能替代事先备份。此 APK 的应用包名是 `io.github.jacek4yang.xhupflow`；不要拿它当 Fcitx5 的包名。

ABI 输出包含 `arm64-v8a` 时，可改下载较小的 `android-arm64.apk`，按第 2 节重新校验，再用相同命令安装。其他支持架构或不确定时用 universal 包；它包含哪些 ABI 应以 Release 的实际构建记录为准。ABI 的含义见 [Android 官方说明](https://developer.android.com/ndk/guides/abis)。

### 3.4 常见错误

- `unauthorized`：解锁手机，确认 USB 调试授权，再运行 `devices -l`。
- 列表为空或 `offline`：检查数据线、USB 口、手机提示及官方驱动；不需要 root。
- `more than one device/emulator`：使用正确的 `-s SERIAL`，不要随机选设备。
- `INSTALL_FAILED_NO_MATCHING_ABIS`：检查 `abilist`，改下载并校验兼容 APK。
- `INSTALL_FAILED_UPDATE_INCOMPATIBLE`：通常是同包名旧应用签名不兼容。**不要直接卸载旧应用**；先保留训练记录、核对旧版来源和签名。优先寻找相同签名的更新；仅在明确接受应用数据丢失并完成可用备份后，才考虑自行卸载。
- `INSTALL_FAILED_VERSION_DOWNGRADE`：目标版本较旧。核对 Release，不要默认增加强制降级选项。
- 手机拒绝安装或被设备管理策略限制：按手机上的说明核对权限和来源；不要关闭安全防护来强行安装。

## 4. Android：安装真正的系统键盘和 Rime 方案

### 4.1 安装 Fcitx5 主程序与 RIME 插件

从 [Fcitx5 for Android 官方安装说明](https://fcitx5-android.github.io/installation/)进入[官方 Releases](https://github.com/fcitx5-android/fcitx5-android/releases)。选择同一个发布版、同一 ABI 的 `app` 和 `plugin.rime` 两个 APK。按官方发布页的哈希/签名信息核对下载文件。已有安装应保持兼容发布渠道，官方明确说明 Google Play 与 GitHub/F-Droid 的签名不通用。

可在电脑把两个已核验的文件分别重命名为 `fcitx5-app.apk`、`fcitx5-rime.apk`，放在第 2 节下载目录，再安装。

Windows：

```powershell
& $Adb -s $Serial install -r '.\fcitx5-app.apk'
if ($LASTEXITCODE -ne 0) { throw 'Fcitx5 主程序安装失败' }
& $Adb -s $Serial install -r '.\fcitx5-rime.apk'
if ($LASTEXITCODE -ne 0) { throw 'RIME 插件安装失败' }
```

macOS/Linux：

```sh
"$ADB" -s "$SERIAL" install -r ./fcitx5-app.apk &&
"$ADB" -s "$SERIAL" install -r ./fcitx5-rime.apk
```

在手机打开 Fcitx5，按应用引导在 Android 设置中启用并选用这个键盘。确认插件已识别，启用 Rime/中州韵附加组件，在“输入法 / Input Methods”中添加 Rime。插件变化需要重载时按界面提示处理。仅安装插件 APK 不等于已经选用 Rime。

### 4.2 把 Rime 文件送到手机的下载目录

先在电脑解压已通过校验的 ZIP，再通过 ADB 复制到公开下载目录。**这一步只是传输文件，还没有安装方案。**

Windows：

```powershell
Set-Location $Work
$Src = Join-Path $Work 'rime-package'
if (Test-Path $Src) { throw 'rime-package 已存在；请先检查旧文件，避免混合两个版本' }
Expand-Archive -LiteralPath $RimeZip -DestinationPath $Src
& $Adb -s $Serial push $RimeZip '/sdcard/Download/'
& $Adb -s $Serial push $Src '/sdcard/Download/'
```

macOS/Linux：

```sh
(
    set -eu
    [ ! -e rime-package ] || { echo 'rime-package 已存在，请先检查旧文件'; exit 1; }
    mkdir rime-package
    unzip "$RIME_ZIP" -d rime-package
    "$ADB" -s "$SERIAL" push "$RIME_ZIP" /sdcard/Download/
    "$ADB" -s "$SERIAL" push rime-package /sdcard/Download/
)
```

手机中会出现 `Download/rime-package`；若已有同名中转目录，先自行改用新的目录名，避免混合版本。工作资料、多用户设备或厂商限制可能使共享存储路径不同；传输被拒绝时使用手机浏览器下载或正常 USB 文件传输，之后继续应用提供的文件访问流程。

### 4.3 在手机备份并复制到正确位置

1. 暂时切到另一个键盘，确保没有正在部署或编辑 Rime 文件。
2. 在 Fcitx5 的 Rime 配置中打开“用户数据目录 / User data dir”。系统文件管理器会打开应用数据入口；进入 `data/rime`。也可通过系统文件管理器 DocumentsUI 的侧栏选择“小企鹅输入法5 / Fcitx 5 for Android”。这一官方入口不需要 root，见 [Fcitx5 FAQ](https://fcitx5-android.github.io/faq/)及[官方 Rime 目录入口实现](https://github.com/fcitx5-android/fcitx5-android/blob/master/app/src/main/java/org/fcitx/fcitx5/android/ui/main/settings/PreferenceScreenFactory.kt)。
3. **在 `data/rime` 之外备份整个 Rime 目录**。另外把原 `default.custom.yaml` 单独保存到例如 `Documents/XHUP-Backups/android-first/`；若不存在，记录“首次安装前不存在”。确认备份可打开。以后更新一直保留这份首次备份，不得用本包配置覆盖它。学习数据库若仍被输入法使用，直接复制不保证一致性；需要保留学习数据时，另按前端支持的导出/备份流程保存，不要把活动数据库的副本当作已验证快照。
4. 打开 `Download/rime-package`，复制全部 `.yaml`、`xhup_flow.sources.tsv` 和 `lua/xhup_flow/` 到 `data/rime`，保留相对目录结构。最终应直接看到 `data/rime/default.custom.yaml`、`data/rime/xhup_flow.schema.yaml`、`data/rime/lua/xhup_flow/init.lua`，不要多套一层 `rime-package` 文件夹。
5. 系统提示重名时，只替换本包对应文件，合并到已有 `lua` 目录；不要删除整个 `lua` 或 Rime 目录。阅读下节的 `default.custom.yaml` 替换影响后再确认。`INSTALL.md`、`NOTICE.md`、`licenses/` 留在下载/备份处，再分发时须保留。
6. 切回 Fcitx5 的 Rime，在键盘状态/方案菜单中执行“部署 / Deploy”，等待完成，再检查 Rime 方案列表。菜单位置随 Fcitx5 版本变化；“同步 / Synchronize”不是部署。部署动作来源见 [fcitx5-rime 官方实现](https://github.com/fcitx/fcitx5-rime/blob/master/src/rimeengine.cpp)。

**不要把这个 ZIP 交给 Fcitx5 的“导入用户数据”功能。** 那是带元数据的完整应用备份恢复，会覆盖设置和输入历史；普通 Rime 源包不符合此格式。见[官方恢复格式实现](https://github.com/fcitx5-android/fcitx5-android/blob/master/app/src/main/java/org/fcitx/fcitx5/android/data/UserDataManager.kt)。

本文不使用 ADB 直接写 Android 受保护的应用目录，不假设普通 ADB 能访问 `/data/data` 或任意 `Android/data`。公开下载目录是中转位置；最终复制通过 Fcitx5 提供的系统文件入口完成。

## 5. 桌面：先装输入法前端，可选安装 Trainer

Rime 前端下载入口：[Windows 小狼毫](https://github.com/rime/weasel/releases)、[macOS 鼠须管](https://github.com/rime/squirrel/releases)、[Linux Fcitx5 安装说明](https://fcitx-im.org/wiki/Install_Fcitx_5)、[IBus-Rime](https://github.com/rime/ibus-rime)。Linux 还需要所选框架的 Rime 引擎包，例如 `fcitx5-rime` 或 `ibus-rime`；按发行版说明启用框架并添加 Rime，不要同时套用多个框架的环境配置。

只安装输入方案可直接跳到第 6 节。Trainer 是可选练习软件：

Windows PowerShell，EXE 与 MSI 二选一：

```powershell
$File = "xhup-flow-trainer-v$AssetVersion-windows-x64-setup.exe"
Get-XhupAsset $File
Start-Process -Wait -FilePath (Join-Path $Work $File)
# 如果改选 MSI，校验该 MSI 后用资源管理器双击，按 Windows 安装向导操作。
```

macOS：

```sh
FILE="xhup-flow-trainer-v$ASSET_VERSION-macos-universal.dmg"
download_xhup "$FILE" && open "$FILE"
```

在打开的 DMG 中按提示将 `XHUP Flow.app` 放到“应用程序”。出现未签名/未公证提醒时，核对来源和 Release 说明；也可暂时只安装手工 Rime 方案，不运行 Trainer。

Debian/Ubuntu x64：

```sh
FILE="xhup-flow-trainer-v$ASSET_VERSION-linux-amd64.deb"
download_xhup "$FILE" &&
dpkg-deb -f "$FILE" Package Version Architecture Depends &&
sudo apt install "./$FILE"
```

Fedora/RPM x64：

```sh
FILE="xhup-flow-trainer-v$ASSET_VERSION-linux-x86_64.rpm"
download_xhup "$FILE" &&
rpm -qp --queryformat '%{NAME} %{VERSION} %{ARCH}\n' "$FILE" &&
sudo dnf install "./$FILE"
```

按发行版正常解析依赖，不使用 `--nodeps`。Linux 包名以包内元数据为准，不把 Rust crate 名 `trainer` 当成必然的系统包名。

## 6. 桌面：手工备份、安装 Rime 源包

### 6.1 替换前必须知道

本包的 `default.custom.yaml` 会替换整个同名文件：Rime 的方案列表变为只显示 `xhup_flow`，原文件中的快捷键等其他全局自定义项也会被替代。其他方案文件、系统键盘和用户学习数据不应被删除。

**以下复制脚本只用于第一次手工安装。** 先在前端中确认实际用户目录，再暂时退出输入法/暂停部署。自定义路径、便携版或 Flatpak 路径不同，不要照抄默认位置。用户目录依据 [Rime 官方说明](https://github.com/rime/home/wiki/RimeWithSchemata)。

旧版若曾通过管理工具安装，应先找到并恢复真正的原始配置备份。不要把已被 XHUP 独占替换的文件误认为首次安装前的原文件，也不要交叉混用手工安装与旧管理工具的文件所有权。

### 6.2 Windows 小狼毫，首次手工安装

继续第 2 节的 PowerShell 窗口。若之前已为 Android 解压过同一已核验版本，可直接使用该 `rime-package` 目录；否则先执行 `Expand-Archive`。

```powershell
$Rime = Join-Path $env:APPDATA 'Rime' # 改成前端实际使用的用户目录
$Src = Join-Path $Work 'rime-package'
if (-not (Test-Path $Src)) { Expand-Archive -LiteralPath $RimeZip -DestinationPath $Src }
if (-not (Test-Path "$Src\xhup_flow.schema.yaml")) { throw '源包位置不正确' }
if (Test-Path "$Rime\.xhup-flow-default-backup.json") { throw '检测到旧管理安装，先恢复原始配置' }
if (Test-Path "$Rime\xhup_flow.schema.yaml") { throw '已有 XHUP；请按更新章节操作' }
$Backup = Join-Path $HOME 'XHUP-Backups\weasel-first'
if (Test-Path $Backup) { throw '首次备份已存在，不得覆盖；请按更新章节操作' }
New-Item -ItemType Directory -Force -Path (Split-Path $Backup) | Out-Null
New-Item -ItemType Directory -Path $Backup | Out-Null
if (Test-Path $Rime) { Copy-Item $Rime "$Backup\rime-userdir" -Recurse }
if (Test-Path "$Rime\default.custom.yaml") {
    Copy-Item "$Rime\default.custom.yaml" "$Backup\default.custom.yaml"
    if ((Get-FileHash "$Rime\default.custom.yaml").Hash -ne (Get-FileHash "$Backup\default.custom.yaml").Hash) {
        throw '首次备份校验失败'
    }
} else { Set-Content "$Backup\original-was-absent.txt" 'default.custom.yaml was absent' }
New-Item -ItemType Directory -Force -Path "$Rime\lua\xhup_flow" | Out-Null
Copy-Item "$Src\*.yaml" $Rime -Force
Copy-Item "$Src\xhup_flow.sources.tsv" $Rime -Force
Copy-Item "$Src\lua\xhup_flow\*" "$Rime\lua\xhup_flow\" -Recurse -Force
Write-Host "首次备份保存在：$Backup"
```

重新启动前端，在小狼毫菜单或开始菜单的“小狼毫 → 重新部署”操作。若需要命令行，先通过前端“程序文件夹”找到实际的 `WeaselDeployer.exe`，再执行 `& '实际完整路径\WeaselDeployer.exe' /deploy`；不要猜测安装版本目录。该参数见[小狼毫官方安装器](https://github.com/rime/weasel/blob/master/output/install.nsi)。

### 6.3 macOS / Linux，首次手工安装

先选择与你实际前端对应的一组变量，三选一：

```sh
# macOS 鼠须管
RIME="$HOME/Library/Rime"; BACKUP="$HOME/XHUP-Backups/squirrel-first"
# Linux Fcitx5：使用这一行时替换上一行
# RIME="${XDG_DATA_HOME:-$HOME/.local/share}/fcitx5/rime"; BACKUP="$HOME/XHUP-Backups/fcitx5-first"
# Linux IBus：以该前端实际用户目录为准
# RIME="${XDG_CONFIG_HOME:-$HOME/.config}/ibus/rime"; BACKUP="$HOME/XHUP-Backups/ibus-first"
```

在第 2 节的下载目录执行：

```sh
(
    set -eu
    if [ ! -d rime-package ]; then mkdir rime-package; unzip "$RIME_ZIP" -d rime-package; fi
    SRC="$PWD/rime-package"
    [ -f "$SRC/xhup_flow.schema.yaml" ] || { echo '源包位置不正确'; exit 1; }
    [ ! -e "$RIME/.xhup-flow-default-backup.json" ] || { echo '先恢复旧管理安装的原始配置'; exit 1; }
    [ ! -e "$RIME/xhup_flow.schema.yaml" ] || { echo '已有 XHUP，请按更新章节操作'; exit 1; }
    mkdir -p "$(dirname "$BACKUP")"
    mkdir "$BACKUP" # 已有首次备份时失败退出，不覆盖
    if [ -d "$RIME" ]; then mkdir "$BACKUP/rime-userdir"; cp -R "$RIME/." "$BACKUP/rime-userdir/"; fi
    if [ -f "$RIME/default.custom.yaml" ]; then
        cp -p "$RIME/default.custom.yaml" "$BACKUP/default.custom.yaml"
        cmp "$RIME/default.custom.yaml" "$BACKUP/default.custom.yaml"
    else
        printf '%s\n' 'default.custom.yaml was absent' > "$BACKUP/original-was-absent.txt"
    fi
    mkdir -p "$RIME/lua/xhup_flow"
    cp "$SRC/"*.yaml "$SRC/xhup_flow.sources.tsv" "$RIME/"
    cp -R "$SRC/lua/xhup_flow/." "$RIME/lua/xhup_flow/"
    printf '首次备份保存在：%s\n' "$BACKUP"
)
```

重新启动前端。鼠须管用菜单“重新部署”，或在鼠须管运行时执行官方支持的命令：

```sh
"/Library/Input Methods/Squirrel.app/Contents/MacOS/Squirrel" --reload
```

来源：[鼠须管官方说明](https://github.com/rime/squirrel)、[官方命令入口](https://github.com/rime/squirrel/blob/master/sources/Main.swift)。Fcitx5/IBus 用户在各自的 Rime 菜单执行“部署 / Deploy”；不要把单纯重启当作所有版本均完成重新编译的保证。

## 7. 更新、回退和卸载

### 方案更新

1. 保留首次 `default.custom.yaml` 备份及“原来不存在”的记录，升级时不能覆盖。
2. 退出输入法，再在另一个带日期的目录备份当前配置和本方案文件；需要备份学习库时，确保输入法已停止使用数据库。
3. 下载并校验新 Rime ZIP，解压到新目录，核对当前 `default.custom.yaml`。安装后若有手工修改，先保留并处理冲突，不要无条件覆盖。
4. 更新包内对应的 YAML、`xhup_flow.sources.tsv` 和 `lua/xhup_flow/`；保留其他方案、`rime.lua`、`user.yaml`、`installation.yaml`、`sync/` 和任何 `*.userdb`。
5. 重新部署，执行下节实测。需要退回上一个 XHUP 版本时，只恢复该版本的方案文件，再部署；不要顺带回滚整个 Rime 用户目录，以免覆盖新学习的数据。

### 退出独占配置或卸载方案

1. 暂时切到其他键盘，停止部署，先备份当前文件。若安装后改过 `default.custom.yaml`，先保留这些新改动。
2. 恢复首次备份的原 `default.custom.yaml`；首次记录为不存在时，移走本包安装的同名文件。首次备份缺失时先停止，不要猜测原配置。
3. 想完整卸载时，只移走已确认属于本包的 `xhup_flow*.schema.yaml`、`xhup_flow*.dict.yaml`、`xhup_flow.sources.tsv` 和 `lua/xhup_flow/`。不要使用 `rm -rf Rime` 或删除整个 `lua/`；不要删除 `xhup_flow_user.userdb`。可把这些文件先移入备份目录，确认恢复成功后再自行决定是否删除。
4. 重新部署，检查原有方案恢复。Android 同样通过 Fcitx5 的系统文件入口操作。

### Trainer 卸载

Trainer 与输入方案独立。若要删除训练应用，先按应用实际支持的方式备份训练记录，再用系统“已安装的应用”卸载，macOS 将应用移到废纸篓，Linux 使用包管理器按前面查询到的实际包名移除。Android 卸载通常会删除应用私有数据，不能靠 `install -r` 恢复已经删除的数据。卸载 Trainer 不等于卸载 Fcitx5、Rime 插件或手工安装的方案。

## 8. 安装后亲自检查

- 在普通文本编辑器中切到 Rime，方案列表应只有 `xhup_flow`。Static 和词典 wrapper 虽然随包保留，不应默认出现在方案菜单里。
- 确认不是英文模式，输入 `uijm` 检查“时间”，并测试 `tiuici`、连续组句、退格、候选选择及最终上屏。候选和文本都必须在真实编辑器中检查。
- 主方案需要满足运行时合同的 `librime-lua`；插件“已安装”或文件存在不能证明所有 API 可用。出现 Lua 错误或“学习暂停”时记录原因，不算完整功能通过。
- 排障需要 Static 时，可在已保留备份的前提下，手工把 `default.custom.yaml` 改为下列内容并重新部署。这是显式退出 Flow 独占配置，不提供 Flow 组句和学习：

```yaml
patch:
  schema_list:
    - schema: xhup_flow_static
```

- 重新启动前端后再测一次。训练器启动成功、APK 安装成功、Rime 部署成功及实际输入/学习成功，是分别要检查的项目。
- 报告问题时附 Release tag、附件名及 SHA-256、系统/前端版本、通过/失败/未测和最小公开复现。不要公开上传学习词库、首次配置备份或私人输入内容。
