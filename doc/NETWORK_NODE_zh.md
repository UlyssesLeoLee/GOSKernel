# K_NET 网络节点说明

`K_NET` 是 GOS 当前的原生网络驱动节点。它负责把 QEMU 提供的虚拟网卡接入图运行时，并把底层硬件状态暴露给 shell。

## 当前能力

- 扫描 PCI 配置空间，识别受支持的网卡
- 识别并优先处理 QEMU `e1000 (8086:100E)`
- 读取并启用 PCI command 位：`io-space` / `memory-space` / `bus-master`
- 解析 BAR，区分 MMIO 和 I/O BAR
- 通过 E1000 的 I/O BAR 读写控制寄存器
- 读取 MAC 地址
- 读取链路状态、速率和双工信息
- 支持重探测和重置初始化

## 当前限制

- 还没有 TX/RX descriptor rings
- 还没有以太网帧发送/接收通路
- 还没有 ARP / DHCP / IP / TCP / UDP
- 对 `virtio-net` 目前只做探测和报告，不做原生数据通路

## Shell 指令

```text
net
net status
net probe
net reset
uplink
```

说明：

- `net` / `net status` / `uplink`：打印当前 uplink 状态
- `net probe`：重新扫描 PCI 并刷新网卡状态
- `net reset`：重新初始化当前网卡寄存器并打印报告

## 典型输出

```text
[NET] uplink status
      transport: qemu virtual nic over host network
      path: guest e1000 -> qemu nat -> host wifi
      pci: 00:03.00 vendor 0x8086 device 0x100E rev 0x03 irq 11
      cmd 0x0007  stage device-ready
      bar: mmio 0xFEBE0000  io 0xC000
      mac: 52:54:00:12:34:56
      carrier: up 1000Mb full-duplex
      stack: nic registers live; tx/rx rings, arp, dhcp, ip pending
```

这里的 `carrier up` 表示虚拟网卡和链路寄存器已经正常，不代表客体系统已经具备完整联网能力。

## 图论定位

`K_NET` 不是单纯的工具函数集合，而是一个有权限声明、导出能力和运行时状态的 driver node：

- capability export：`net/uplink`
- shell 通过 edge 与它交互
- `net` 指令本质上是向该 node 发送控制信号
- 设备状态由 node 自己维护，不由 shell 硬编码
