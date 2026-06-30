# GOS 裸机安装指南

本文档描述的是“在目标机器上没有 Rust、Cargo、QEMU 开发环境”的安装路径。

## 当前安装模型

现阶段 GOS 提供的是可启动原始磁盘镜像，而不是图形化安装器。

这意味着最简单的安装方式是：

1. 在一台构建机上生成 `gos-installer.img`。
2. 把镜像写入 U 盘。
3. 用目标机器从该 U 盘启动。

如果要把系统直接装入目标机器硬盘，本质上也是把同一个原始镜像写到目标磁盘，只是风险更高，因此脚本默认优先面向 U 盘。

## 方案 A：直接下载 CI 产物

如果仓库的 GitHub Actions 已经跑过 `installer-artifact` 工作流，可以直接下载预编译安装包：

- `gos-installer-release.zip`

解压后会得到：

- `gos-installer.img`
- `installer-manifest.json`
- `write-usb-image.ps1`
- 本说明文档

这条路径最适合“什么都没有的目标机器”，因为目标机器本身不需要任何开发工具。

## 方案 B：在构建机本地生成安装包

### 1. 准备构建机

构建机需要：

- Rust nightly
- `rust-src`
- `llvm-tools-preview`
- `cargo bootimage`
- PowerShell 7

建议命令：

```powershell
rustup toolchain install nightly
rustup component add rust-src llvm-tools-preview --toolchain nightly
cargo install bootimage --locked
```

### 2. 生成安装包

在仓库根目录执行：

```powershell
pwsh -File .\tools\build-installer.ps1 -Profile release
```

成功后会在 `dist\gos-installer` 下得到安装镜像和元数据。

## 把镜像写入 U 盘

### 1. 先列出磁盘

```powershell
pwsh -File .\tools\write-usb-image.ps1 -List
```

记下目标 U 盘的 `DiskNumber`。

### 2. 写入镜像

```powershell
pwsh -File .\tools\write-usb-image.ps1 -ImagePath .\dist\gos-installer\gos-installer.img -DiskNumber 3
```

注意：

- 必须以管理员身份运行 PowerShell。
- 脚本默认拒绝系统盘和启动盘。
- 脚本默认只允许可移动磁盘；固定磁盘需要显式 `-Force`。

## 在目标机器上启动

1. 插入写好镜像的 U 盘。
2. 进入 BIOS/UEFI 启动菜单。
3. 选择该 U 盘启动。
4. GOS 会引导进入当前的 builtin graph，并进入由 supervisor 持续服务的系统控制台。

## 当前限制

这套安装链已经解决了“目标机器不需要开发环境”的问题，但还没有完成以下能力：

- 系统内交互式安装器
- 自动分区和磁盘选择 UI
- 首次启动向导
- 持久化网络配置向导

也就是说，当前版本更接近“可启动交付镜像”，而不是传统发行版那种安装程序。

## 故障排查

### `cargo bootimage` 失败

如果在 Windows 上遇到 `llvm-objcopy.exe: permission denied`，优先使用 CI 产物，或者改在 Linux/WSL/GitHub Actions 上构建安装包。

### U 盘写入失败

如果 `write-usb-image.ps1` 无法独占目标磁盘，可以改用外部写盘工具，例如 Rufus 或 balenaEtcher，把 `gos-installer.img` 写入 U 盘。

### 目标机器无法启动

请检查：

- BIOS/UEFI 是否允许从 USB 启动
- 目标机器是否识别该 U 盘
- 镜像写入是否完整
- 下载的安装包 SHA256 是否与 `installer-manifest.json` 一致
