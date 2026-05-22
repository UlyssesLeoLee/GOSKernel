# GOS Kernel — Plugin Interface Catalog (YAML)

每个 GOS 插件用一个 YAML 文件描述它对外承诺的接口契约，
作为 Rust `BuiltinPluginDescriptor` 的人类可读 mirror。
外部工具（审计、文档生成、可视化、依赖图分析、AI 助手）
可以直接消费这些 YAML 而不用读 Rust 源码。

## 文件命名

`interfaces/<plugin_id_lowercase>.yaml`

例如：`interfaces/k_ps2.yaml`、`interfaces/k_cypher.yaml`。

## 模式 (schema)

```yaml
plugin_id: K_PS2                  # 大写下划线 ASCII (PluginId::from_ascii)
name: k-ps2                       # crate 名 / 显示名
version: 1                        # plugin 自身的 semver major
abi_version: "0.2.0"              # GOS_ABI_VERSION (packed semver)
node_type: Driver                 # RuntimeNodeType
sub_domain: KernelDriver          # NodeSubDomain (J.6 ACL 用)
state_schema_hash: "0x2008"       # NodeSpec.state_schema_hash (u64 十六)
entry_policy: Bootstrap           # EntryPolicy
executor_id: native.ps2           # ExecutorId

# 该插件向外暴露的能力（J.4 版本化）
exports:
  - namespace: shell
    name: input
    version: 1

# 该插件需要从其他插件 import 的能力（J.4 版本范围）
imports:
  - namespace: shell
    capability: input
    required: true
    min_version: 1
    max_version: 4294967295        # u32::MAX

# 该插件依赖的其他插件 (depends_on)
depends_on:
  - K_PIC

# 该插件请求/获得的硬件权限
permissions:
  - kind: PortIo
    arg0: "0x60"
    arg1: "0x64"
  - kind: IrqBind
    arg0: 1
    arg1: 0

# 该插件在 manifest 里静态声明的图边（manifest_edges_well_formed
# P2 #5 验证过）。Cypher 运行时动态创建的边不出现在这里。
edges: []

# 该插件注册的运行时节点
nodes:
  - local_key: ps2.entry
    vector: "0.4.0.0"             # CORE_PS2 (vectors::CORE_PS2)
    routes:                       # I.3.x 条件路由
      - key: 0x00
        target: shell.entry
      - key: 0x01
        target: ime.entry
```

## 维护规则

1. 这些 YAML 是 **代码的 mirror，不是 source of truth**。
   规范定义在 Rust `BuiltinPluginDescriptor` 里。
2. 修改 plugin manifest 时同步更新对应 YAML（CI 检查可在 L+ 阶段加）。
3. `version` 升级遵循 J.4 capability-versioning 规则：
   * 兼容修改：bump version
   * 破坏修改：bump version + 老 import 用 max_version 卡住

## 用途

* **AI 助手** 读 YAML 理解插件结构，不需要爬 Rust 源
* **依赖图可视化**：所有 YAML 一起喂给 graphviz / mermaid
* **审计**：扫 YAML 找出有 `kind: PortIo / IrqBind / GraphWrite` 等高权限的插件
* **文档**：生成 plugin reference 文档
* **K.4 ARCHITECTURE.md** 引用具体插件时用相对路径链接

## 当前覆盖

21 个 builtin 插件全部有对应 YAML。未来用户态 ELF 插件用同样的 schema
描述（vector 字段空，由加载器分配）。
