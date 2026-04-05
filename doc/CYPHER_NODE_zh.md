# GOS Cypher Node 手册

`K_CYPHER` 是一个原生 graph node。它提供受控的 Cypher v1 子集，用于：

- 浏览 node
- 浏览 edge
- 激活 node
- 生成 node
- 路由 edge

它不是完整 Neo4j Cypher，也不是通用图写入解释器。

## 一、入口方式

### 显式前缀

```text
cypher MATCH (n) RETURN n
```

### 直接输入

```text
MATCH (n) RETURN n
```

## 二、当前支持的语法子集

### 2.1 节点浏览

```text
MATCH (n) RETURN n
MATCH (n) RETURN n LIMIT 6
MATCH (n {vector:'6.1.0.0'}) RETURN n
MATCH (n {vector:'0xffff806001000000'}) RETURN n
```

### 2.2 边浏览

```text
MATCH ()-[e]-() RETURN e
MATCH ()-[e]-() RETURN e LIMIT 6
MATCH (n {vector:'6.1.0.0'})-[e]-() RETURN e
MATCH ()-[e {vector:'e:17.34.51.68'}]-() RETURN e
```

### 2.3 节点控制

```text
MATCH (n {vector:'6.1.0.0'}) CALL activate(n)
MATCH (n {vector:'6.1.0.0'}) CALL spawn(n)
```

### 2.4 边控制

```text
MATCH ()-[e {vector:'e:17.34.51.68'}]-() CALL route(e)
```

## 三、当前语义边类型

`RETURN e` 当前会如实显示 runtime 中存在的语义边，包括：

- `depend`
- `call`
- `spawn`
- `signal`
- `return`
- `mount`
- `sync`
- `stream`
- `use`

这意味着你可以直接用 Cypher 观察：

- `theme.current -[use]-> theme.*`
- `node -[mount]-> clipboard.mount`

## 四、主题与剪贴板示例

### 4.1 查看当前主题状态节点

```text
MATCH (n {vector:'6.1.3.0'}) RETURN n
```

### 4.2 查看当前主题使用边

```text
MATCH (n {vector:'6.1.3.0'})-[e]-() RETURN e
```

### 4.3 通过激活主题节点切换主题

```text
MATCH (n {vector:'6.1.1.0'}) CALL activate(n)
MATCH (n {vector:'6.1.2.0'}) CALL activate(n)
```

这会刷新 `theme.current` 的排他 `Use` 关系。

### 4.4 查看剪贴板挂载节点

```text
MATCH (n {vector:'6.1.4.0'}) RETURN n
```

### 4.5 查看谁挂载了剪贴板

```text
MATCH (n {vector:'6.1.4.0'})-[e]-() RETURN e
```

## 五、向量写法

### 节点向量

- `6.1.0.0`
- `0xffff806001000000`

### 边向量

- `e:17.34.51.68`
- `e:0xffff811022033044`

## 六、安全边界

当前严格限制为：

- 必须以 `MATCH` 起手
- 只支持 `RETURN n`
- 只支持 `RETURN e`
- 只支持 `CALL activate(n)`
- 只支持 `CALL spawn(n)`
- 只支持 `CALL route(e)`

当前明确 **不支持**：

- `CREATE`
- `DELETE`
- `SET`
- `MERGE`
- 事务
- 任意属性修改
- 任意 edge 创建或删除

因此，Cypher node 是一个 **图浏览与受控执行入口**，不是通用图数据库写接口。

## 七、最短示例

```text
MATCH (n) RETURN n
MATCH (n {vector:'6.1.3.0'})-[e]-() RETURN e
MATCH (n {vector:'6.1.1.0'}) CALL activate(n)
MATCH (n {vector:'6.1.4.0'})-[e]-() RETURN e
```
