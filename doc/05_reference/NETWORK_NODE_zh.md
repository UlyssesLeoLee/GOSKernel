# K_NET 网络节点说明

`K_NET` 是 GOS 当前的原生网络 driver node。它负责把 QEMU 提供的虚拟网卡接入当前 graph-native 运行时，并把底层链路状态暴露给 shell 和其他消费者。

## 一、当前能力

- 扫描 PCI 配置空间，识别受支持的网卡
- 优先处理 QEMU `e1000 (8086:100E)`
- 读取并启用 PCI command 位：`io-space` / `memory-space` / `bus-master`
- 解析 BAR，区分 MMIO 与 I/O BAR
- 通过 E1000 I/O BAR 读写控制寄存器
- 读取 MAC 地址
- 读取链路状态、速率与双工信息
- 支持重探测与重置初始化

## 二、当前限制

- 还没有 TX/RX descriptor rings
- 还没有以太网帧发送/接收通路
- 还没有 ARP / DHCP / IP / TCP / UDP
- 对 `virtio-net` 当前只做探测与报告，不做数据通路

## 三、图论定位

`K_NET` 不是命令工具函数集合，而是 builtin graph 中的原生 driver node：

- capability export：`net/uplink`
- shell 通过 import/export 与 `Mount` 关系消费它
- `net` 指令本质上是向该 node 发送控制信号
- 设备状态由 node 自己维护，不由 shell 硬编码

这意味着：

- 网络状态是图中的 node 状态
- 网络依赖关系可以通过 graph summary 或 cypher 被观察
- 后续网络扩展也应继续走 capability + edge 模型

## 四、Shell 指令

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

## 五、典型输出

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

这里的 `carrier up` 只表示虚拟网卡与链路寄存器已经正常，不代表客体系统已经具备完整联网协议栈。

## 六、后续演进方向

`K_NET` 的后续扩展应服从当前路线图：

1. 先完成底座清理与原子化执行模型
2. 再把 `K_NET` 接到更稳定的资源、实例、fault 恢复模型
3. 最后补 TX/RX rings、协议栈、上层网络服务
