# aliasc

[English](README.md) | [中文](README.zh-CN.md)

`aliasc` 把一份纯文本别名源文件编译成快速的、按 shell 生成的启动工件——支持
bash、zsh、sh/dash、fish、nushell、PowerShell 和 cmd。

**解决的问题。** 别名会随机器和 shell 增殖，每种 shell 语法各不相同
（`alias x=...` / `def x` / `Set-Alias` / doskey），而可移植的别名没法表达
"装了 `bat` 就用 `bat`，否则用 `cat`" 这类逻辑。aliasc 用单一事实源，
把解析成本一次性转移到编译期：shell 启动只 source 一份生成的原生定义文件——
不解析 DSL、不起子进程、不在启动期探测候选命令。

```
~/.config/alias   （一份 DSL 文件，所有 shell 共用）
        │  aliasc compile --shell <shell> --platform <platform>
        ▼
~/.cache/aliasc/alias.<shell>                 ← shell 实际加载的文件
~/.cache/aliasc/alias.<shell>.manifest.json   ← 新鲜度元数据
```

## 安装

### Homebrew（macOS、Linux x64）

本仓库本身就是 Homebrew tap：

```sh
brew tap dodorz/aliasc https://github.com/dodorz/aliasc
brew install aliasc
```

安装会把编译器和 glue 脚本一起放进 `$(brew --prefix)/opt/aliasc/libexec`，
因此一切——包括你修改别名源后的重新编译——都完全离线。往 profile 加一行
（路径随升级自动更新）：

```sh
# bash / zsh / sh
. "$(brew --prefix)/opt/aliasc/libexec/alias.sh"
```

fish：`source .../alias.fish` · nushell：`source .../alias.nu` ·
PowerShell：`. .../alias.ps1`

### Release bundle（任意平台，无包管理器）

每个 release 发布一份按平台的 `.zip`，内含编译好的二进制加 glue：

```sh
unzip -d aliasc aliasc-x86_64-unknown-linux-gnu.zip
cd aliasc
./alias-install.sh --shell bash --profile ~/.bashrc   # 拷贝 glue，写入激活行
```

```powershell
Expand-Archive aliasc-x86_64-pc-windows-msvc.zip -DestinationPath aliasc
cd aliasc
.\alias-install.ps1 -Shell pwsh -Profile $PROFILE
```

`./alias-install.sh --no-compiler` 只装 glue；fetcher 会在首次 shell 启动时
下载二进制。裸二进制也发布在 zip 旁边（供引导路径和 Scoop manifest 使用）。

每个 bundle 都自包含：`alias-fetch.sh` 会找到旁边的编译器，从不联网。

## 别名源文件

`~/.config/alias`（可用 `ALIAS_FILE` 覆盖）。格式：`name=body`，按
`[Section]` 分组，按目标 platform、distro、environment 选择生效段。

```ini
include "shared.alias"          # 相对于所在文件展开

[Common]                        # 所有目标
greet=printf "hello ${USER}\n" @1
cat=?(bat, cat)                 # FirstAvailable 简写：装了 bat 用 bat，否则 cat
pip=WithEnv(PIP_REQUIRE_VIRTUALENV=false) pip @*
work=SetEnv(HTTP_PROXY=http://127.0.0.1:7890)
nowork=UnsetEnv(HTTP_PROXY HTTPS_PROXY)
pipe=grep -c @@                 # @@ 是字面 @

[UNIX]                          # Linux + macOS
[macOS] [Ubuntu] [Debian] [Fedora] [Arch] [Windows] [WSL] [GitBash] [MSYS2]
```

- **Section** 是 platform/distro/environment 选择器，不按 shell 选：
  `Common`、`UNIX`、`macOS`、`Windows`、`Ubuntu`、`Debian`、`Fedora`、`Arch`、
  `WSL`、`GitBash`、`MSYS2`。同名定义（大小写不敏感）后写者胜。
  `[Bash]` 之类 shell 段保留待用。
- **参数**：`@1`…`@9`、`@*`（全部参数）。body 里没有任何 `@` 占位符时自动
  追加 `@*`。
- **模板**：`FirstAvailable(a, b, …)` 在**首次调用时**解析出第一个已安装的
  命令（每会话缓存）；`SetEnv(...)` / `UnsetEnv(...)` / `WithEnv(...)` 管理
  环境变量作用域；body 里的 `\cmd` 强制调用外部命令，避免递归进别名自身。
- **多行定义**：括号未闭合时定义可跨行——`FirstAvailable` 每行一个候选最
  好读。其内部忽略空行、`#`/`;` 注释和行尾 `#` 注释。
- 主源文件旁的 `alias.local` 自动追加（机器本地覆盖）；`ShortcutMap.yaml`
  （见下）是另一个可选输入。
- 名字解析大小写不敏感；生成的入口覆盖精确拼写及其全小写形式。

## ShortcutMap（可选）

一个 YAML 文件（默认 `$HOME/.config/ShortcutMap.yaml`，否则用别名源旁边的），
**编译期**展开为普通定义——是数据，不是运行时机制：

```yaml
Config:
  Foo: ~/.foo            # → EditFooConfig（用 ${EDITOR} 打开）

History:
  Bash: ~/.bash_history

Log:
  App: /var/log/app.log  # → ViewAppLog（用 ${LOGVIEWER}）

aliases:                  # 自由定义逃生舱
  Custom: printf custom

Forward:                  # 从 ~/.ssh/config 读取 Host 条目
  Syncthing: 8384         # → ForwardMacSyncthing = ssh -N -L 8384:... mac
  apps:
    Jellyfin:
      remote_port: 8096
      local_port: 18096
```

主源文件与 `alias.local` 的同名定义覆盖 ShortcutMap 生成的定义。值一律当作
数据处理并加引号后再插入 DSL；Windows 风格路径的反斜杠保持字面不变。

## CLI

```sh
aliasc compile --shell posix --platform linux --source ~/.config/alias --output out.sh
aliasc validate --shell pwsh --platform windows --source C:\path\to\alias
aliasc list --shell bash --platform linux --source ...        # 名字（--format tsv：名字<TAB>body）
aliasc search --shell bash --platform linux --source ... proxy
aliasc explain --shell bash --platform linux --source ... pkg  # 某个别名展开成什么
aliasc run update -- --deep            # 在任何 shell 之外执行某个定义
```

### 在 shell 之外运行别名

Shell 函数不会跨越进程边界，直接 exec 程序的父进程（`sudo`、`env`、`xargs`、
cron、`ssh host cmd`）都看不见你的别名。`aliasc run NAME [-- args...]` 弥补
这个缺口：它按当前目标上下文解析定义（生效段、`FirstAvailable` 探测、
`WithEnv`），然后在全新的 `sh` 里执行。`aliasc` 本身就是 PATH 上的真实
二进制，所以 `sudo` 乐意运行它：

```sh
sudo aliasc run update-system
sudo aliasc run pip -- install -U rich
sudo "$(command -v aliasc)" run backup   # 当 brew bin 不在 sudo 的 secure_path 里时
```

`--source` 默认取 `$ALIAS_FILE`，再退到 `~/.config/alias`；子进程退出码原样
传播。`SetEnv`/`UnsetEnv` 定义没有可执行命令，会被拒绝。（远端主机是部署
问题：需要远端自己装 aliasc 或同步 shim。）

`--shell` 接受 `posix`、`bash`、`zsh`、`fish`、`nu`、`powershell`/`pwsh`、
`cmd`。`powershell`/`pwsh` 要求 Windows 目标上下文；任意宿主机都支持交叉
编译（`--platform`、`--distro`、`--environment` 默认自动探测；
`--no-local` / `--shortcut-map` / `--no-shortcut-map` 控制额外输入）。每次
编译都会写一份 manifest，记录解析出的上下文、被跟踪的输入（mtime）与内容
哈希。

## Shell 集成

每个 shell 的 profile 只 source 一个文件；它把缓存的工件加载进**当前会话**，
并先清除上一代定义：

| Shell | profile 行 | 工件 |
| --- | --- | --- |
| sh, bash, zsh, dash | `. /path/to/aliasc/alias.sh` | `alias.posix` |
| fish | `source /path/to/aliasc/alias.fish` | `alias.fish` |
| nushell | `source /path/to/aliasc/alias.nu` | `alias.nu` |
| PowerShell 5.1 / 7+ | `. /path/to/aliasc/alias.ps1` | `alias.pwsh` |

glue 文件（见下）必须放在同一个目录；每个 activator 优先找同目录的兄弟
文件，找不到再退到 `${XDG_DATA_HOME:-~/.local/share}/aliasc`（设
`ALIASC_LIB_DIR` 直接指定）。dash 无法报告被 source 的路径，所以从 dash 直接
source 时只有数据目录候选有效——把 glue 装在那里，或 export
`ALIASC_LIB_DIR`。

### glue 脚本

| 文件 | 职责 |
| --- | --- |
| `alias.sh` / `alias.fish` / `alias.nu` / `alias.ps1` | 各 shell 的 activator：fetch → 新鲜度检查 → 把工件加载进当前会话。加载是 shell 私有的，所以每个 shell 一个文件。 |
| `alias-core.sh` | 共享 POSIX 核心：定位编译器、按 manifest 判断新鲜度、编译、把工件路径打到 stdout。自己从不加载任何东西。 |
| `alias-fetch.sh` | 下载器：`--print-binary-path` 报告缓存路径；缺失时下载匹配平台的二进制（带 glibc/musl 检测、wget 兜底），缓存在 `${XDG_CACHE_HOME:-~/.cache}/aliasc/bin`。 |
| `alias-fallback.sh` | 仅 POSIX 的降级解析器：既拿不到编译器也没有工件时，用纯 `sh` 解析 DSL，保证断网也不会让 shell 失去别名。 |
| `alias-install.sh` / `alias-install.ps1` | bundle 安装器：把 glue（连同 bundle 内编译器）拷进一个目录并写激活行；`--uninstall` 逐字节还原 profile。 |

`alias.ps1` 不能依赖 `/bin/sh`，所以它在 PowerShell 里自行完成 core/fetcher
步骤（设 `ALIASC_BINARY` 可完全跳过下载）；fish/nu 的 activator 直接通过
`sh` 调 `alias-core.sh`。

### 降级路径

| 情况 | POSIX | fish / nu / PowerShell |
| --- | --- | --- |
| 编译器在、工件新鲜 | 加载工件 | 加载工件 |
| 编译器在、工件过期 | 重编译后加载 | 重编译后加载 |
| 下载失败、有工件 | 复用工件 | 复用工件 |
| 下载失败、无工件 | `alias-fallback.sh` 解析 DSL | 报错，不加载别名 |

只有 POSIX 会降级成解析器：fallback 靠 `eval` POSIX 函数体工作，其他 shell
无法消费。

## 绕过别名

每个工件都定义了统一的逃生舱：

```sh
raw ls --all      # 运行外部 ls，忽略别名
```

各 shell 的原生写法：`command ls`（POSIX shell、fish）、`^ls`（nushell）、
`ls.exe` 或 `& (Get-Command ls -CommandType Application).Path`（PowerShell）、
`cmd /c ls`（cmd）。POSIX shell 上 `\ls` **不是**绕过——aliasc 生成的是
函数，反斜杠只影响 alias 展开，所以它仍会执行别名函数体。

对于函数无法跨越的进程边界（`sudo`、cron、`xargs`），使用 `aliasc run`——
见[在 shell 之外运行别名](#在-shell-之外运行别名)。

## 环境变量

| 变量 | 含义 |
| --- | --- |
| `ALIAS_FILE` | 别名源路径（默认 `~/.config/alias`） |
| `ALIASDSL_NOLOAD=1` | activator 直接退出、不加载（用于一次性测试） |
| `ALIASC_CACHE_DIR` | 工件缓存（默认 `${XDG_CACHE_HOME:-~/.cache}/aliasc`） |
| `ALIASC_LIB_DIR` | glue 所在目录（activator 通常自行定位） |
| `ALIASC_CORE`、`ALIASC_FALLBACK`、`ALIASC_FETCHER` | 显式指定核心、fallback 解析器、fetcher 路径 |
| `ALIASC_BINARY` | 使用现有编译器，跳过下载（PowerShell activator） |
| `ALIASC_BINARY_CACHE_DIR` | 编译器下载缓存（默认 `.../aliasc/bin`） |
| `ALIASC_REPOSITORY`、`ALIASC_RELEASE` | 下载源（默认 `dodorz/aliasc`、`latest`） |
| `ALIASC_PLATFORM` | PowerShell activator 的平台覆盖 |
| `ALIASC_SSH_CONFIG` | ShortcutMap `Forward` 段读取的 SSH 配置 |

## 从源码构建

```sh
cargo build --release        # target/release/aliasc
cargo test                   # 单元测试 + fixture 驱动的 shell 集成测试
```

Rust stable；运行时依赖仅 `clap` 与 `serde`/`serde_yaml`。

## 发布

GitHub Actions 在每次 push 时构建 Linux x64（glibc + musl）、macOS
Intel/Apple Silicon、Windows x64/x86/ARM64、Android ARM64（Termux）；与
`Cargo.toml`（唯一版本事实源）精确匹配的 `MAJOR.MINOR.PATCH` tag（可加 `v`
前缀）会发布包含全部二进制、按平台 zip bundle 和 `checksums.txt` 的
release。`Cargo.lock` 必须携带相同版本，否则 `--locked` 构建失败。由于本仓库
默认分支里的 Homebrew formula 指向 release 资产，务必**先打 tag 发版，再提交
formula 的版本/校验和更新**。

## 许可证

MIT
