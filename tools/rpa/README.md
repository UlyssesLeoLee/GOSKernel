# GOS Kernel — Windows RPA 工具集

PowerShell 脚本集合，用于在 Windows 上自动化 QEMU + GOS 内核的整套操作。
所有脚本走 QEMU 的 `-monitor telnet:127.0.0.1:45555` 和 `-serial tcp:127.0.0.1:14444` 已经在 `crates/hypervisor/Cargo.toml` 的 `run-args` 里配好的 TCP 接口，不依赖 SendKeys / GUI 模拟，更稳定。

## 脚本清单

| 脚本 | 用途 |
|---|---|
| `qemu-launch.ps1` | 后台启动 QEMU，返回 PID |
| `qemu-quit.ps1` | 通过 monitor 优雅关机（`system_powerdown`，超时后 `quit`） |
| `qemu-monitor.ps1` | 发任意 HMP monitor 命令到 QEMU，回显输出 |
| `qemu-sendkey.ps1` | 通过 monitor 的 `sendkey` 发送按键序列 |
| `qemu-screenshot.ps1` | 通过 monitor 的 `screendump` 抓 PPM，转 PNG |
| `qemu-serial-read.ps1` | 读 serial TCP 流，可 tail/filter |
| `qemu-cypher.ps1` | 高层封装：发 Cypher 命令并读响应 |
| `qemu-smoke.ps1` | 端到端 smoke：launch → wait boot → screenshot → quit |
| `qemu-bench.ps1` | 自动化 `BENCH RPC` 多轮取均值 |

## 公共依赖

所有脚本共用 `_common.ps1` 里的 helper：
- `Connect-QemuMonitor` — 建立 telnet TCP 连接，读到 prompt
- `Send-QemuMonitor` — 发送一行 HMP 命令，读响应直到下一个 prompt
- `Send-QemuSerial` — 注入字节到 serial TCP（模拟键盘输入）
- `Read-QemuSerial` — 读 serial 缓冲（用于 grep boot 标记）

## 端口约定

| 用途 | 端口 |
|---|---|
| Monitor (HMP/QMP) | 127.0.0.1:45555 |
| Serial COM1 | 127.0.0.1:14444 |
| (默认未启用) Serial COM2 | — |

如改了 `crates/hypervisor/Cargo.toml` 的 `run-args`，对应改 `_common.ps1` 顶部的常量。

## 用法示例

```powershell
# 启动 QEMU 并等待 boot 完成
& .\qemu-launch.ps1
& .\qemu-serial-read.ps1 -WaitForMarker 'enabling interrupts'

# 截图
& .\qemu-screenshot.ps1 -OutPath '.\boot.png'

# 执行 Cypher 查询，把结果打到 serial 流
& .\qemu-sendkey.ps1 -Keys 'kernel' -Then Enter
& .\qemu-sendkey.ps1 -Keys 'show stats' -Then Enter
Start-Sleep -Seconds 1
& .\qemu-screenshot.ps1 -OutPath '.\after-stats.png'

# 优雅关机
& .\qemu-quit.ps1
```

## 完整自动化场景示例

`qemu-smoke.ps1` 串起整个流程：build → launch → wait boot → screenshot → quit。
适合 CI 或者你想"无人值守"地验证某个改动是否破坏视觉输出。

## 已知限制

- QEMU 在 Windows 上的 `screendump` 输出 PPM；脚本会用 ImageMagick (`magick`) 转 PNG。如果没装 ImageMagick，screenshot 会留 .ppm 文件。
- `sendkey` 一次只能发一个 key code。多键序列由 PowerShell 端 loop 拼装。
- 中文输入：通过 serial inject UTF-8 字节（`Send-QemuSerial`），但 GOS 内核当前只识别 ASCII。
