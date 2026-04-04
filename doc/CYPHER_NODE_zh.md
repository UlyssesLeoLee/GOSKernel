# GOS Cypher Node v1 手册

`K_CYPHER` 是一个原生 graph node。它把一组受控的 Cypher v1 子集接进了 GOS shell，用于在命令行里浏览和控制 node / edge。

这不是完整 Neo4j Cypher；它是面向 GOS runtime 的可执行子集，重点是可控、可验证、可落地。

## 入口方式

- 显式前缀：

```text
cypher MATCH (n) RETURN n
```

- 直接输入 Cypher：

```text
MATCH (n) RETURN n
```

## 当前支持的语法子集

### 节点浏览

```text
MATCH (n) RETURN n
MATCH (n) RETURN n LIMIT 6
MATCH (n {vector:'6.1.0.0'}) RETURN n
MATCH (n {vector:'0xffff806001000000'}) RETURN n
```

说明：

- `RETURN n` 会列出节点或显示单节点详情。
- `LIMIT` 当前只对列表查询生效，范围会被限制在安全页宽内。

### 边浏览

```text
MATCH ()-[e]-() RETURN e
MATCH ()-[e]-() RETURN e LIMIT 6
MATCH (n {vector:'6.1.0.0'})-[e]-() RETURN e
MATCH ()-[e {vector:'e:17.34.51.68'}]-() RETURN e
```

说明：

- 全局边列表会按 `EdgeVector` 稳定排序。
- 带 node 向量过滤时，会返回该节点的全部入边和出边。
- 带 edge 向量过滤时，会直接显示该边详情。

### 节点控制

```text
MATCH (n {vector:'6.1.0.0'}) CALL activate(n)
MATCH (n {vector:'6.1.0.0'}) CALL spawn(n)
```

说明：

- `CALL activate(n)` 直接调用 runtime 的激活路径。
- `CALL spawn(n)` 固定发送 `Spawn { payload: 0 }`。

### 边控制

```text
MATCH ()-[e {vector:'e:17.34.51.68'}]-() CALL route(e)
```

说明：

- `CALL route(e)` 会按该 edge 的语义类型调用 runtime 路由。
- `Call` 边会发送 `Call { from: k-cypher }`。
- 其他可执行边默认走 `Spawn { payload: 0 }`。
- 这是第一版的安全默认值，不开放任意 payload。

## 向量写法

- 节点向量：
  - `6.1.0.0`
  - `0xffff806001000000`
- 边向量：
  - `e:17.34.51.68`
  - `e:0xffff811022033044`

## 最短示例

```text
MATCH (n) RETURN n
MATCH (n {vector:'6.1.0.0'}) RETURN n
MATCH (n {vector:'6.1.0.0'})-[e]-() RETURN e
MATCH ()-[e {vector:'e:17.34.51.68'}]-() CALL route(e)
```

## 安全边界

- 当前只支持 `MATCH` 起手的查询。
- 当前只支持 `RETURN n`、`RETURN e`、`CALL activate(n)`、`CALL spawn(n)`、`CALL route(e)`。
- 不支持 `CREATE`、`DELETE`、`SET`、`MERGE`、事务、变量绑定回写、任意属性修改。
- 这意味着它是一个“图控制台节点”，不是一个通用图数据库解释器。
