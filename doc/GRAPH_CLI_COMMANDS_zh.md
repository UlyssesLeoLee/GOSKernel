# GOS Graph CLI 指令手册

本文档描述当前 `k-shell` 内建的图控制终端能力。  
口径只记录当前真实实现，不描述尚未支持的未来语法。

如果你希望通过 Cypher 风格浏览相同的 runtime 图，请看 [CYPHER_NODE_zh.md](./CYPHER_NODE_zh.md)。

## 一、向量写法

### 节点向量

- 图坐标：`6.1.0.0`
- Canonical 十六进制：`0xffff806001000000`

### 边向量

- 图坐标：`17.34.51.68`
- 兼容前缀：`e:17.34.51.68`
- Canonical 十六进制：`0xffff811022033044`

## 二、图上下文语义

- 初始状态没有当前图上下文。
- `show` 会进入 overview。
- `node <vector>` 会进入 node 详情。
- `show` 在 node 上下文里会切到该 node 的 edge 列表。
- `edge <vector>` 会进入 edge 详情。
- `show` 在 edge 上下文里会切到该 edge 关联的 node 视图。
- `back` 会退回上一层图视图。

## 三、核心图命令

| 指令 | 作用 |
|---|---|
| `show` | 初始时进入 overview；在 node / edge 上下文里切换另一侧视图 |
| `show next` | 当前 overview / list 下一页 |
| `show prev` | 当前 overview / list 上一页 |
| `node <vector>` | 选中并显示一个 node |
| `edge <vector>` | 选中并显示一个 edge |
| `node` | 显示当前已选 node 详情 |
| `edge` | 显示当前已选 edge 详情 |
| `where` | 显示当前 node / edge 选择状态 |
| `back` | 返回上一层 graph 视图 |
| `select clear` | 清空选择与图上下文 |
| `activate` | 激活当前选中 node |
| `spawn` | 向当前选中 node 发送 `Spawn { payload: 0 }` |

### 翻页与历史

- `PgUp`：图视图上一页
- `PgDn`：图视图下一页
- `Up`：上一条命令历史
- `Down`：下一条命令历史，回到底时恢复当前草稿

## 四、主题图命令

当前终端主题通过图里的真实关系表达：

- `6.1.1.0` -> `theme.wabi`
- `6.1.2.0` -> `theme.shoji`
- `6.1.3.0` -> `theme.current`

真正生效的关系始终是：

- `theme.current -[use]-> theme.wabi`
- 或 `theme.current -[use]-> theme.shoji`

### 主题命令

| 指令 | 作用 |
|---|---|
| `theme` | 显示当前主题状态与主题节点 |
| `theme wabi` | 让 `theme.current -[use]-> theme.wabi` |
| `theme shoji` | 让 `theme.current -[use]-> theme.shoji` |

### 图方式切换主题

```text
node 6.1.1.0
activate
```

或：

```text
node 6.1.2.0
activate
```

这里 `activate(theme.*)` 的效果不是直接修改 shell 私有变量，而是刷新 `theme.current` 的排他 `Use` 关系，然后立即切显示调色板。

## 五、共享剪贴板命令

当前共享剪贴板是独立的图节点：

- `6.1.4.0` -> `clipboard.mount`

它的关系是非排他的 `Mount`：

- 任意多个 node 都可以同时 `-[mount]-> clipboard.mount`

默认 builtin graph 会把以下节点挂到它上面：

- `shell.entry`
- `cypher.query`
- `ai.supervisor`

### 剪贴板命令

| 指令 | 作用 |
|---|---|
| `clipboard` | 显示 `clipboard.mount` 状态和当前挂载边 |
| `clipboard clear` | 清空共享剪贴板内容 |
| `clipboard mount <vector>` | 给某个 node 增加 `-[mount]-> clipboard.mount` |
| `clipboard unmount <vector>` | 删除某个 node 到 `clipboard.mount` 的挂载边 |
| `clip clear` | `clipboard clear` 别名 |
| `clip mount <vector>` | `clipboard mount` 别名 |
| `clip unmount <vector>` | `clipboard unmount` 别名 |

### 剪贴板快捷键

在当前输入缓冲区或 API 编辑器里：

- `Ctrl+C`：复制当前输入
- `Ctrl+X`：剪切当前输入
- `Ctrl+V`：粘贴共享剪贴板内容

这些快捷键只有在当前节点已经挂载 `clipboard.mount` 时才会生效。

## 六、Cypher、网络、CUDA、AI 入口

| 指令 | 作用 |
|---|---|
| `cypher <query>` | 把受控 Cypher v1 查询发给 `k-cypher` |
| `MATCH ...` | 直接输入 Cypher，无需前缀 |
| `net` / `net status` / `uplink` | 查看 `k-net` 当前 uplink 状态 |
| `net probe` | 重新扫描 PCI 并刷新网卡状态 |
| `net reset` | 重新初始化当前网卡寄存器并打印状态 |
| `cuda` / `cuda status` | 查看 host-backed CUDA bridge 状态 |
| `cuda submit <job>` | 提交一条 host-backed job |
| `cuda demo` | 发送示例 job |
| `cuda reset` | 重置 bridge 计数和捕获状态 |
| `ai` | 进入底栏 AI API 编辑器 |
| `ask <prompt>` | 发送 prompt 到 AI chat lane |
| `Ctrl+L` | 切换 IME 语言模式 |

## 七、输出说明

### `show`

overview 会同时显示：

- node 摘要
- edge 摘要

### `node <vector>`

node 详情至少会显示：

- vector
- plugin / local key
- type
- lifecycle
- entry policy
- executor id
- export count

### `show` 在 node 上下文中

会显示关联 edge 列表，格式类似：

```text
<dir> <edge-vector> <edge-type> <from-vector> -> <to-vector>
```

如果是 capability 挂载边，会额外显示：

```text
cap=<namespace/name>
```

### `edge <vector>`

edge 详情至少会显示：

- edge vector
- edge type
- from / to 向量与 local key
- route policy
- ACL
- capability 绑定
- edge id

## 八、最短示例

### 浏览主题关系

```text
node 6.1.3.0
show
```

### 切换主题

```text
theme shoji
theme
```

### 浏览剪贴板挂载关系

```text
clipboard
node 6.1.4.0
show
```

### 给一个 node 挂载剪贴板

```text
clipboard mount 6.1.0.0
clipboard
```
