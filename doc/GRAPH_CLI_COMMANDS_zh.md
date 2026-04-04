# GOS Graph CLI v1 指令手册

GOS v0.2 现已内建基础图控制台。左侧主命令区可以直接在 node / edge 两种视角之间切换，并用显式向量命令控制选择。

如果你希望用 Cypher 风格来做相同的图浏览和控制，请继续看 [CYPHER_NODE_zh.md](./CYPHER_NODE_zh.md)。

## 向量语法

- 节点向量支持两种输入：
  - 图坐标：`6.1.0.0`
  - Canonical 十六进制：`0xffff806001000000`
- 边向量在 `edge <vector>` 命令里可以直接写：
  - 图坐标：`17.34.51.68`
  - 兼容写法：`e:17.34.51.68`
  - Canonical 十六进制：`0xffff811022033044`

## 上下文语义

- 初始状态没有当前图上下文。
- 这时输入 `show`，会进入 overview，同时显示当前页的 node 和 edge 摘要。
- 输入 `node <vector>` 后，当前上下文变成 `node`，会显示该节点详情。
- 在 `node` 上下文里输入 `show`，会切到该节点的关联 edge 视图。
- 输入 `edge <vector>` 后，当前上下文变成 `edge`，会显示该边详情。
- 在 `edge` 上下文里输入 `show`，会切到该边关联的 node 视图。

也就是说，`show` 现在是一个“上下文切换”命令，不再只是固定列 node。

## 指令表

| 指令 | 作用 |
|---|---|
| `show` | 初始时显示 overview；在 node / edge 上下文里切换到另一侧 |
| `show next` | 当前 overview / list 下一页 |
| `show prev` | 当前 overview / list 上一页 |
| `node <vector>` | 选中并显示一个 node |
| `edge <vector>` | 选中并显示一个 edge |
| `node` | 显示当前已选 node 详情 |
| `edge` | 显示当前已选 edge 详情 |
| `where` | 显示当前选中的 node / edge |
| `select clear` | 清空 node / edge 选择和图上下文 |
| `activate` | 调用当前选中 node 的 `activate` 路径 |
| `spawn` | 向当前选中 node 发送 `Spawn { payload: 0 }` |

## 翻页

- `PgUp`：当前图视图上一页
- `PgDn`：当前图视图下一页
- 文本兼容指令 `show next` / `show prev` 仍然可用

当前支持翻页的视图：

- overview
- node 列表
- edge 列表

## 输出说明

### 初始 `show`

会显示一个 overview：

- 上半部分是 node 摘要
- 下半部分是 edge 摘要

每页同时翻动 node 和 edge 的当前页窗口。

### `node <vector>`

会进入 node 详情视图，至少显示：

- vector
- plugin name / plugin id
- local key
- node type
- lifecycle
- entry policy
- executor id
- export count

### `show` 在 node 上下文中

会进入该 node 的关联 edge 列表，每行格式：

```text
<dir> <edge-vector> <edge-type> <from-vector> -> <to-vector>
```

如果该边由 capability mount 生成，会附加：

```text
cap=<namespace/name>
```

### `edge <vector>`

会进入 edge 详情视图，显示：

- edge vector
- edge type
- from / to 向量和 local key
- route policy
- ACL
- capability 绑定
- edge id

### `show` 在 edge 上下文中

会切到该 edge 对应的 node 视图，显示该边两端 node 的摘要。

## 选择状态

- `node <vector>` 会更新 `sel-node`
- `edge <vector>` 会更新 `sel-edge`
- `edge <vector>` 不会清掉已有 node 选择；它会让当前图上下文变成 `edge`
- `select clear` 会同时清空 `sel-node`、`sel-edge` 和图上下文

## 安全控制边界

- `activate` 只作用于当前 `sel-node`
- `spawn` 只作用于当前 `sel-node`，并固定发送 `Spawn { payload: 0 }`
- 第一版不提供任意 payload 的边路由命令
- 第一版不在 Graph CLI 中直接开放边执行；边执行控制优先通过 Cypher node 提供

## 最短操作示例

```text
show
node 6.1.0.0
show
edge 17.34.51.68
show
where
```

含义如下：

1. `show` 先进入 graph overview
2. `node 6.1.0.0` 选中 shell 节点并显示详情
3. `show` 切到这个节点的 edge 列表
4. `edge 17.34.51.68` 选中某一条 edge 并显示详情
5. `show` 切到这条 edge 关联的 node 视图
6. `where` 查看当前选择状态
