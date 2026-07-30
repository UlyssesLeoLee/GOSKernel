# GOS Graph CLI 指令手册

| 项目 | 内容 |
|---|---|
| 文档编号 | GOS-DOC-03-04 |
| 所属阶段 | 03・详细设计 |
| 版本 / 状态 | v1.10 / 现行（口径：仅记录已实现命令） |
| 作成 / 审核 / 批准 | GOS 核心团队 |
| 基线日期 | 2026-06-30 |
| 最终更新 | 2026-07-29 |

**变更履历**

| 版本 | 日期 | 变更内容 | 作成者 |
|---|---|---|---|
| v1.10 | 2026-07-29 | 补齐 V3.126 新增的 topo115（NOCTAENNACTC + NHOCTAENNACTC + NBFFSO，八旬系列第10个/最终，NB 系列第 32 个字母组 FF，α=166），更新 §十五表头、引言统计（61 次硬化迭代）与累计口径（2231 项测试、L4 占用至 202）；将 gos-graph-topo115-harness 10 项测试收入版本记录 | GOS 核心团队 |
| v1.9 | 2026-07-29 | 对照硬化日志及源码提交历史补齐 V3.105~V3.125（21 次硬化迭代）新增的 Neighborhood S-variant 拓扑指数命令族 `graph topo94`~`graph topo114`（21 组、63 个指数），扩展 §十五表格与统计口径；新发现 topo99（V3.110）、topo100（V3.111）、topo102（V3.113）三版本硬化日志文件缺失——本轮已用 `git log --all --oneline` 核实对应 `feat(vX.XXX)` 提交确实存在（代码与测试均已合入，仅文档缺失），如实标注并附提交哈希；同时用同一方法核实此前标记为「待核实」的 V3.66、V3.102 缺口，结论相同（均为文档侧遗漏，非代码缺失或误删），解除该待跟进项；发现 V3.117 硬化日志自述累计测试数「2141」与 V3.116 重复（应为 2151），经核对 V3.118 的起始基数确认为 V3.117 落款时误抄上一版本数值，如实标注不擅自修正 | GOS 核心团队 |
| v1.8 | 2026-07-21 | 对照硬化日志补齐 V3.66~V3.104（39 项硬化迭代，其中 V3.66、V3.102 两版本文件缺失）新增的 Neighborhood S-variant 拓扑指数命令族 `graph topo55`~`graph topo93`（39 组、117 个指数），新增 §十五完整索引，原 §十五/§十六 顺延为 §十六/§十七（本行为补记：此前 v1.8 发布时遗漏在本表登记变更履历，本轮一并补齐） | GOS 核心团队 |
| v1.7 | 2026-07-19 | 对照源码及硬化日志补齐 V3.31~V3.65（共 35 项）Neighborhood S-variant 拓扑指数命令族 `graph topo20`~`graph topo54`（105 个指数），新增 §十四，原 §十四/§十五（输出说明/最短示例）顺延为 §十五/§十六。此前 v1.6 索引截至 V3.30（graph topo19），是本次修订前口径滞后最严重的部分（README.md 已连续多轮标记为高优先级待处理项） | GOS 核心团队 |
| v1.0 | 2026-06-30 | 纳入日系工程阶段目录（03_详细设计） | GOS 核心团队 |
| v1.1 | 2026-07-01 | 补充文档管理信息 | GOS 核心团队 |
| v1.2 | 2026-07-01 | 对照 `crates/k-shell/src/proc.rs` 源码核对命令口径，发现 V2.8~V2.14 新增的 `nodes` `edges` `graph diff` `journal` `metrics export` `boot verify` `proc/ps` 等命令此前未收录，补充新增 §七「运维与可观测性命令」 | GOS 核心团队 |
| v1.3 | 2026-07-02 | 对照源码补齐 V2.16~V2.42 新增的图拓扑 / 进程管理 / 图论分析命令族（`graph topo` `graph health` `kill` `resume` `node info` `node trace` `node log` `uname` `watch` `graph path` `graph cycles` `graph toposort` `graph scc` `graph condensation` `graph reachable` `graph bipartite` `graph degree` `graph centrality` `graph closeness` `graph eccentricity` `graph katz` 等），新增 §八「进程与拓扑运维扩展」、§九「图论分析命令族」，原 §八/§九 顺延为 §十/§十一；确认 V2.42 `graph katz` 版本号并移出待核实项 | GOS 核心团队 |
| v1.4 | 2026-07-03 | 对照源码补齐 V2.43~V2.65 新增的第二代图论分析命令族（`graph pagerank` `graph hits` `graph community` `graph spanning` `graph color` `graph mst` `graph shortest` `graph flow` `graph between` `graph attractor`）与属性存储/图健康度命令族（`node attr` `node attr list` `node attr list u8` `graph density` `graph clustering` `graph transitivity` `graph kcore` `graph assortativity`），新增 §十「第二代图论与图健康度命令族」，原 §十/§十一（输出说明/最短示例）顺延为 §十一/§十二 | GOS 核心团队 |
| v1.5 | 2026-07-06 | 对照源码及 `doc/06_运维维护/hardening/` 下 V2.66~V3.06（共 41 项）硬化日志补齐网络科学扩展指标、图分析工具化、第三代结构分解与经典图论算法套件三条命令族，新增 §十一「网络科学与图分析工具化命令族」、§十二「第三代结构分解与经典图论算法套件」，原 §十一/§十二（输出说明/最短示例）顺延为 §十三/§十四 | GOS 核心团队 |
| v1.6 | 2026-07-15 | 对照源码及硬化日志补齐 V3.07~V3.30（共 24 项）：连通性/边染色/谱分析/信息熵/经典 Zagreb 四件套，以及 `graph topo`~`graph topo19` 拓扑指数命令族（19 组、57 个分子图拓扑描述符），新增 §十三「谱分析与拓扑指数命令族」，原 §十三/§十四（输出说明/最短示例）顺延为 §十四/§十五。此前 v1.5 索引截至 V3.06，V3.07 之后长期未同步，是本次修订前口径滞后最严重的部分 | GOS 核心团队 |

---

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

## 七、运维与可观测性命令（V2.8 ~ V2.14 新增）

> 本节内容核对自 `crates/k-shell/src/proc.rs` 命令分发表（`dispatch_shell_command`），随源码新增命令同步更新。

### 7.1 节点 / 边巡检（ps / ss 风格）

| 指令 | 作用 |
|---|---|
| `nodes` / `nodes all` | 列出全部 node（ps 风格） |
| `nodes faulted` / `nodes fault` / `faults` | 仅列出 Faulted 状态 node |
| `nodes summary` / `nodes stat` | node 生命周期状态汇总 |
| `edges` / `edges all` | 列出全部 edge（ss 风格） |
| `edges count` / `edge count` | 边计数统计 |
| `edges <type>` | 按类型过滤边；`<type>` ∈ `call spawn depend signal return mount sync stream use` |
| `stat <vector>` / `node stat <vector>` | 显示指定 node 的统计详情 |
| `proc` / `ps` / `proc all` | 进程风格列表：vector / 信号计数 / 出边数 / 生命周期状态（V2.14 新增，详见 [hardening/HARDENING_LOG_2026-07-01_V2.14.md](../06_运维维护/hardening/HARDENING_LOG_2026-07-01_V2.14.md)） |

### 7.2 拓扑变更日志（git-diff 风格）

| 指令 | 作用 |
|---|---|
| `graph diff` / `diff` / `diff graph` | 显示自 baseline epoch 以来的拓扑变更（node/edge 增删） |
| `graph diff <epoch>` / `diff <epoch>` | 显示自指定 epoch（十进制）起的拓扑变更，不改变当前 baseline（V2.16 新增） |
| `graph diff pin` / `diff pin` | 把当前 epoch 设为新的 diff baseline |
| `graph diff reset` / `diff reset` | baseline 重置为 epoch 0（显示自 boot 以来全部变更） |

详细设计见 [ADR-004](./ADR-004-mutation-visibility.md) 与 V2.13 硬化日志。

### 7.3 遥测、日志与启动校验

| 指令 | 作用 |
|---|---|
| `metrics export` / `metrics dump` | 导出机器可解析的 `key=value` 遥测快照 |
| `journal` / `journal status` / `journal info` | 显示 journal ring 状态与格式信息 |
| `boot` / `boot verify` / `boot status` | 显示 boot manifest 边校验报告 |
| `modules` | 列出 supervisor 已安装模块及状态、故障策略、重启计数、DEGRADED 标记 |

---

## 八、进程与拓扑运维扩展（V2.16 ~ V2.30 新增）

> 本节内容对照 `crates/k-shell/src/proc.rs` 命令分发表核实，随源码新增命令同步更新。

### 8.1 拓扑与健康巡检

| 指令 | 作用 |
|---|---|
| `graph topo` / `topo` | 按 L4 domain 统计节点计数（V2.17 新增） |
| `graph topo <L4>` / `topo <L4>` | 列出指定 L4 domain（0~255）下的全部节点 |
| `graph health` / `health` | 汇总故障节点数、diff 环填充率、订阅对数、抢占 / 域切换次数、启动校验结果为一份健康横幅（V2.18 新增） |
| `plugins` / `lsmod` / `plugin list` | 列出全部已注册插件的名称、版本、加载状态与节点数（V2.20 新增，lsmod 风格） |

### 8.2 节点生命周期管理

| 指令 | 作用 |
|---|---|
| `kill <vector>` / `node fault <vector>` / `fault <vector>` | 强制指定节点进入 Faulted 状态（V2.21 新增，相当于 `kill -9`） |
| `resume <vector>` / `node resume <vector>` | 恢复故障 / 挂起节点为 Ready，使其可再次接收信号（V2.22 新增） |
| `node info <vector>` / `ninfo <vector>` | 单节点综合视图，等价于 `stat` + 该节点出边与入边列表（V2.23 新增） |
| `node trace <vector>` / `ntrace <vector>` | 显示该节点最近的信号分发记录（seq / kind / cmd / from），最新在前（V2.24 新增，类似 `strace -p`） |
| `node trace clear <vector>` / `ntrace clear <vector>` | 清空该节点的信号追踪环形缓冲区，不影响累计信号计数（V2.27 新增） |
| `node log <vector>` / `nlog <vector>` | 显示该节点最近的生命周期状态迁移记录（tick + 状态标签），最新在前（V2.25 新增，类似 `journalctl -u`） |
| `node log clear <vector>` / `nlog clear <vector>` | 清空该节点的生命周期日志环形缓冲区（V2.26 新增） |
| `node stat clear <vector>` / `nstat clear <vector>` | 将 `stat` / `proc` 显示的累计信号计数清零（V2.28/V2.29 新增） |
| `uname` / `uname -a` / `ver` / `version` | 显示内核版本信息（V2.28/V2.29 新增） |

### 8.3 实时监视

| 指令 | 作用 |
|---|---|
| `watch` / `graph watch` / `watch proc` / `watch nodes` | 进入实时监视模式，每个心跳刷新一次 VECTOR DECK 的 proc 面板，任意按键退出（V2.30 新增，类似 `watch -n1 proc`） |
| `watch stop` / `watch exit` | 退出实时监视模式 |

## 九、图论分析命令族（V2.31 ~ V2.41 新增）

> 本节命令均为只读图算法查询，不修改运行时图状态；对照源码核实，口径仅记录已实现命令。

| 指令 | 算法 | 作用 |
|---|---|---|
| `graph path <from> <to>` | BFS | 计算并逐跳打印两个 vector 地址之间沿有向边的最短路径（V2.31 新增） |
| `graph cycles` / `cycles` / `graph cyclic` / `cyclic` | DFS 环检测 | 检测图中的有向环，打印每个环从入口节点回到自身的路径（V2.32 新增） |
| `graph toposort` / `toposort` / `topo sort` / `graph tsort` / `tsort` | Kahn BFS | 拓扑排序；若存在环则输出部分排序并提示先运行 `graph cycles`（V2.33 新增） |
| `graph scc` / `scc` / `graph components` / `components` | Kosaraju | 求强连通分量；分量数等于节点数时说明图为 DAG（V2.34 新增） |
| `graph condensation` / `condensation` / `condense` / `graph condense` | SCC 缩点 | 将每个 SCC 缩为超节点（`C#N`），展示缩点后的 DAG 及分量间边（V2.35 新增） |
| `graph reachable <vector>` / `reachable <vector>` / `reach <vector>` / `graph reach <vector>` | DFS | 从指定 vector 出发，列出可传递到达的全部节点（V2.36 新增） |
| `graph bipartite` / `bipartite` / `graph bip` / `bip` | BFS 二染色 | 检测图是否为二分图（V2.37 新增） |
| `graph degree` / `degree` / `graph hub` / `hub` | 度统计 | 统计每个节点的入度 / 出度，识别孤立节点与枢纽节点（V2.38 新增） |
| `graph centrality` / `centrality` / `graph central` / `central` / `betweenness` | Brandes | 计算介数中心性，找出承载最多跨服务通信路径的节点（V2.39 新增） |
| `graph closeness` / `closeness` / `graph close` / `close centrality` / `cc` | 出向紧密中心性 | 衡量节点以最少平均跳数广播到其余节点的能力（V2.40 新增） |
| `graph eccentricity` / `eccentricity` / `graph ecc` / `ecc` / `graph radius` / `radius` | BFS 全源最短路 | 计算每个节点的偏心率，并在同一输出中给出图的半径（radius）与直径（diameter）（V2.41 新增；命令行无独立 `diameter` 别名，直径作为该命令输出字段呈现） |
| `graph katz` / `katz` / `kz` / `graph influence` / `influence` | Katz 中心性 | 入向 Katz 中心性——对所有长度的有向游走计数，比最短路径类指标（closeness / eccentricity）更全面地刻画节点间接影响力（V2.42 新增，图论算法套件 V2.32~V2.42 收官） |

## 十、第二代图论与图健康度命令族（V2.43 ~ V2.65 新增）

> 本节命令均为只读查询，不修改运行时图状态（属性写入类命令 `node attr set` 除外，详见 10.3）；对照源码核实，口径仅记录已实现命令。详细算法说明见对应硬化日志（`doc/06_运维维护/hardening/`）。

### 10.1 排名与结构分析命令族

| 指令 | 算法 | 作用 |
|---|---|---|
| `graph pagerank` / `pagerank` / `pr` / `graph rank` / `rank` | PageRank | 随机游走稳定分布，归一化权威性排名（V2.43 新增） |
| `graph hits` / `hits` / `graph ha` / `ha` / `hub authority` | Kleinberg HITS | hub/authority 二部图分解，区分"转发者"与"被引用目标"（V2.44 新增） |
| `graph community` / `community` / `lpa` / `graph lpa` / `graph cluster` / `cluster` | 标签传播（LPA） | 社区发现，识别紧耦合子系统分组（V2.45 新增） |
| `graph spanning` / `spanning` / `span` / `graph span` / `graph tree` / `gtree` | BFS 生成森林 | 展示连接全部活跃节点的最小无环骨架（V2.46 新增） |
| `graph color` / `color` / `gcolor` / `graph colour` / `colour` | Welsh-Powell 贪心着色 | 无冲突调度域划分，输出色度数（V2.47 新增） |
| `graph mst` / `mst` / `gmst` / `graph tree mst` / `min spanning` | Prim 最小生成森林 | 带权最小成本连通骨架（V2.48 新增，引入 `edge_weight` 快照基础设施） |
| `graph shortest <vec>` / `shortest <vec>` / `graph dijkstra <vec>` / `dijkstra <vec>` | Dijkstra | 单源最短路径树（有向、带权）（V2.49 新增） |
| `graph flow <src> <snk>` / `flow <src> <snk>` / `max flow` / `maxflow` | Edmonds-Karp | 两节点间最大流（V2.50 新增） |
| `graph between` / `between` / `gbetween` / `graph wbc` / `wbc` / `weighted betweenness` | Brandes + Dijkstra | 加权介数中心性，识别关键中继节点（V2.53 新增） |
| `graph attractor` / `attractor` / `gattractor` / `graph attract` / `attract` | Kosaraju + 缩点 | 吸引子集合分类（attractor / drain / transient）（V2.54 新增） |
| `graph sim` / `sim` / `gsim` / `graph walk` / `walk` / `graph sim <N>` 等 | xorshift32 随机游走 | 模拟 N 步随机游走并统计各节点访问计数（V2.52 新增） |

### 10.2 图健康度与结构指标命令族

| 指令 | 算法 | 作用 |
|---|---|---|
| `graph density` / `density` / `gdensity` | E/(N·(N-1)) | 图密度（ppm），衡量整体稀疏度（V2.59 新增） |
| `graph clustering` / `clustering` / `gcluster` | Watts-Strogatz | 全局聚类系数（ppm），衡量局部三角化程度（V2.61 新增） |
| `graph transitivity` / `transitivity` / `gtrans` | 三角形/三元组比值 | 与聚类系数同公式，额外暴露原始三角形/三元组计数（V2.63 新增） |
| `graph kcore` / `kcore` / `gkcore` / `graph core` / `core decomp` / `coreness` | Batagelj-Zaversnik 剥离 | k-核分解：每节点核度 + 图退化度（core/inner/periphery 角色）（V2.64 新增） |
| `graph assortativity` / `assortativity` / `gassort` | Newman (2002) | 度同配系数 r∈[-1,+1]，衡量高度节点是否倾向连接高度节点（V2.65 新增） |

### 10.3 节点属性存储命令族（PAL_U32 图原生化重构，V2.55~V2.62）

| 指令 | 作用 |
|---|---|
| `node attr set <vec> <hex>` / `nattr set <vec> <hex>` | 设置节点的 u32 属性值（V2.55 新增） |
| `node attr get <vec>` / `nattr get <vec>` / `node attr <vec>` | 读取节点的 u32 属性值（V2.55 新增） |
| `node attr list` / `nattr list` | 枚举所有已设置 u32 属性的节点（V2.58 新增） |
| `node attr list u8` / `nattr list u8` | 枚举所有已设置 u8 属性的节点（V2.60 新增，与 u32 表对称；路由需在 `node attr list` 前优先匹配） |

该命令族是调色板从硬编码常量 `PAL_U32` 迁移到图原生节点属性的基础设施：V2.55 建立存储原语 → V2.56 引导时为 `theme.wabi`/`theme.shoji` 写入颜色 → V2.57 渲染路径改为读取节点属性 → V2.62 补齐 `palette.cyan`/`palette.gold` 两个节点，四项调色板条目全部图原生化。详见 [hardening/HARDENING_LOG_2026-07-03_V2.55.md](../06_运维维护/hardening/HARDENING_LOG_2026-07-03_V2.55.md) 起各篇。

## 十一、网络科学与图分析工具化命令族（V2.66 ~ V2.84 新增）

> 本节命令均为只读查询，不修改运行时图状态（`graph snapshot save` 除外，其写入的是独立快照存储而非运行时图本身）；对照源码及对应硬化日志核实。

### 11.1 网络科学指标命令族

| 指令 | 算法 | 作用 |
|---|---|---|
| `graph reciprocity` / `grecip` | 互反边比值 | 有向边互反率：`A->B` 且同时存在 `B->A` 的比例（V2.66 新增） |
| `graph modularity` / `gmodq` | Newman Q 公式 | 基于 V2.45 LPA 社区划分计算模块度 Q，衡量社区结构强度（V2.67 新增） |
| `graph rich club` / `grichclub` | Rich-club 系数 | 高度节点之间连接密度相对随机基线的比值，检测"核心圈子"结构（V2.68 新增） |
| `graph girth` / `ggirth` | BFS 最短环 | 图中最短有向环的长度（V2.69 新增） |
| `graph wiener` / `gwiener` | 全源 BFS 距离和 | Wiener 指数：全部可达节点对最短距离之和，衡量图的"整体延展度"（V2.70 新增） |
| `graph harmonic` / `gharm` | 调和中心性 | 到其余节点距离倒数之和，对不可达节点友好的中心性指标（V2.71 新增） |
| `graph peripheral` / `gperiph` | 偏心率边界 | 偏心率等于图直径的节点集合，即结构最外围节点（V2.72 新增） |
| `graph center` / `gcenter` | 偏心率边界 | 偏心率等于图半径的节点集合，即结构中心节点，与 peripheral 对称互补（V2.73 新增） |
| `graph efficiency` | 逆距离归一化均值 | 全局效率 E(G)，网络科学中衡量信息传播效率与鲁棒性的核心指标（V2.74 新增） |
| `graph avg clustering` | Watts-Strogatz | 图的真实平均聚类系数（逐节点局部聚类系数的无权平均），区别于 §10.2 的全局聚类系数（V2.75 新增） |
| `graph local efficiency` | 子图 BFS | 局部效率：每个节点邻域子图的全局效率均值（V2.76 新增） |
| `graph small world` | σ = (C/C_rand)/(L/L_rand) | 小世界系数 σ，判定图是否兼具高聚类与短路径的小世界特征（V2.77 新增） |
| `graph scale free` | 幂律拟合判定 | 基于度分布判断图是否呈现无标度（scale-free）特征（V2.78 新增） |
| `graph power law` | 最大似然估计 (MLE) | 幂律指数 γ̂ 的最大似然估计，量化度分布的幂律拟合程度（V2.80 新增；V2.81 起集成进 `graph summary` 面板） |
| `graph summary` / `gsummary` | 汇总面板 | 图拓扑一站式报告：整合密度/聚类/效率/小世界/无标度/幂律指数等指标于单一面板（V2.79 新增，V2.81 补充幂律指数） |
| `graph diameter` / `gdiameter` | center + peripheral 组合 | 一站式结构边界视图：同时展示图半径、直径、中心节点与外围节点（V2.82 新增） |

### 11.2 图分析工具化命令族

| 指令 | 作用 |
|---|---|
| `graph snapshot` / `graph snapshot save` | 将当前图指标（密度/聚类/效率等）保存为一份带时间戳的快照（V2.83 新增） |
| `graph snapshot list` | 列出已保存的历史快照（V2.83 新增） |
| `graph compare` / `gcompare` / `graph watch compare` | 对比当前图指标与指定历史快照的差值，量化拓扑演化趋势（V2.83~V2.84 新增） |
| `graph predict` | 链路预测：并列输出 Common Neighbors（CN）、Jaccard、Adamic-Adar、Resource Allocation 四种指标，预测最可能出现的新链接（V2.84 新增） |

## 十二、第三代结构分解与经典图论算法套件（V2.85 ~ V3.06 新增）

> 本节命令均为只读图算法查询，覆盖经典图论中一系列结构分解、连通性判定与 NP-hard 问题的精确/近似算法；对照源码及 `doc/06_运维维护/hardening/` 下对应硬化日志核实，口径仅记录已实现命令。

| 指令 | 算法 | 作用 |
|---|---|---|
| `graph articulation` | Tarjan | 求割点（cut vertices）：移除后会增加连通分量数的节点（V2.85 新增） |
| `graph bridges` | Tarjan | 求割边（bridges）：移除后会增加连通分量数的边（V2.86 新增） |
| `graph eulerian` | 度数奇偶校验 + Hierholzer 思路 | 检测欧拉路径 / 欧拉回路是否存在（V2.87 新增） |
| `graph dag longest` | DAG 动态规划 | DAG 最长路径 / 关键路径分析（V2.88 新增） |
| `graph dag layers` | Kahn 分层 | 拓扑层级划分，标识可并行执行的节点层（V2.89 新增） |
| `graph domtree` | Cooper–Harvey–Kennedy 2001 | 支配树构建，识别控制流意义上的必经节点（V2.90 新增） |
| `graph feedback arc` / `graph fas` | DFS 三染色 | 反馈弧集（打破所有环所需移除的最少边集合）（V2.91 新增） |
| `graph bipartite match` | Kuhn 匈牙利算法 | 二分图最大匹配（V2.92 新增） |
| `graph 2ecc` / `g2ecc` | Tarjan 边连通扩展 | 2 边连通分量划分：对单条边故障具备容错能力的最大子图分组（V2.93 新增） |
| `graph truss` | k-truss 逐层剥离 | k-truss 分解，返回每条边的 truss 层级（V2.94 新增） |
| `graph clique` | 迭代 Bron-Kerbosch + Tomita pivot | 最大团检测（V2.95 新增） |
| `graph indep` | BK 补图法 | 最大独立集（在补图上运行团检测得到）（V2.96 新增） |
| `graph vertex cover` / `gvc` | König 定理 + 2-近似 | 最小顶点覆盖（二分图精确解，一般图 2-近似）（V2.97 新增） |
| `graph dominating set` | 贪心 ln(Δ)+1 近似 | 最小支配集：满足"每个未入选节点至少与一个入选节点相邻"的近似最小集合（V2.98 新增） |
| `graph min path cover` | König / Dilworth | DAG 最小路径覆盖：覆盖全部节点所需的最少条点不相交路径（V2.99 新增） |
| `graph arborescence` | Chu-Liu / Edmonds 1967 | 以指定根节点为起点的最小生成树形图（有向 MST）（V3.00 新增） |
| `graph fvs` | 贪心 Kahn 法 | 最小反馈点集：打破所有环所需移除的最少节点集合（V3.01 新增） |
| `graph min cut` | Stoer-Wagner 1997 | 全局最小割：使图变为不连通所需切断的最小权重边集合（V3.02 新增） |
| `graph hamiltonian` / `graph ham` | 迭代回溯 DFS | Hamiltonian 路径 / 回路检测（V3.03 新增） |
| `graph chordal` | LexBFS + PEO 验证 | 弦图识别：判断图是否为弦图（每个长度 ≥4 的环都存在弦）（V3.04 新增） |
| `graph bcc` | Tarjan 迭代边栈法 | 双连通分量（biconnected components）划分（V3.05 新增） |
| `graph ebc` | Brandes (2001) 边版本 | 边介数中心性：识别承载最多最短路径流量的关键边（V3.06 新增，与 §10.1 节点介数中心性构成完整 betweenness 族） |

累计至 V3.06，宿主测试总数为 **1033 个**。详细算法说明、复杂度分析与测试用例见对应硬化日志（`doc/06_运维维护/hardening/HARDENING_LOG_*.md`）。

## 十三、谱分析与拓扑指数命令族（V3.07 ~ V3.30 新增）

> 本节命令均为只读图算法查询。V3.07~V3.11 为连通性/染色/谱分析/信息论/经典指数补完；V3.12~V3.30 为化学图论「拓扑指数」（topological index）命令族的持续扩展，每组固定新增 3 个指数并配套一个 `gos-graph-topoN-harness`（10 项测试）。口径仅记录已实现命令，对照源码及 `doc/06_运维维护/hardening/` 下对应硬化日志核实；本节为 2026-07-15 本轮新增，此前版本（v1.5 及更早）未收录 V3.07 之后的命令，属于本文档当前最大的口径滞后补齐。

### 13.1 连通性、染色、谱与信息论补完（V3.07 ~ V3.11）

| 指令 | 算法 | 作用 |
|---|---|---|
| `graph vconn` | Even (1975) 节点分裂最大流 | 点连通度 κ(G)：使图不连通所需删除的最少节点数（V3.07 新增） |
| `graph ecolor` | 贪心 Vizing 算法 | 边染色 χ'(G)：为每条边分配颜色使相邻边不同色所需的最少颜色数（V3.08 新增） |
| `graph spectral` | 邻接矩阵/拉普拉斯矩阵幂迭代 | 谱半径 ρ(A) + 代数连通度 λ₂(L)（Fiedler 值），衡量图的谱结构与连通鲁棒性（V3.09 新增） |
| `graph entropy` / `gentropy` | Shannon (1948) | 度数分布香农熵 H(G) 及归一化熵 H'，衡量图的结构多样性（V3.10 新增） |
| `graph zagreb` / `gzagreb` | Gutman & Trinajstić (1972) 等 | 经典拓扑指数四件套：第一/第二 Zagreb 指数 M1/M2、Randić 连通性指数 R、Albertson 不规则指数 I（V3.11 新增） |

### 13.2 拓扑指数命令族（`graph topo` ~ `graph topo19`，V3.12 ~ V3.30）

| 组号 | 指令 | 版本 | 新增指数 |
|---|---|---|---|
| topo1 | `graph topo` / `gtopo` | V3.12 | SC（Sum-Connectivity）+ GA（Geometric-Arithmetic）+ AZI（Augmented Zagreb） |
| topo2 | `graph topo2` / `gtopo2` | V3.13 | H（Harmonic）+ ABC（Atom-Bond Connectivity）+ F（Forgotten） |
| topo3 | `graph topo3` / `gtopo3` | V3.14 | SDD（Symmetric Division Degree）+ ISI（Inverse Sum Indeg）+ Nirmala |
| topo4 | `graph topo4` / `gtopo4` | V3.15 | Sombor + RM₂（约化第二 Zagreb）+ Sigma |
| topo5 | `graph topo5` / `gtopo5` | V3.16 | HM₁ + HM₂（第一/第二超-Zagreb）+ AG（算术-几何指数） |
| topo6 | `graph topo6` / `gtopo6` | V3.17 | EM₁（重构第一 Zagreb）+ ABS（原子-键和连通性）+ RRR（约化倒数 Randić） |
| topo7 | `graph topo7` / `gtopo7` | V3.18 | W（Wiener）+ H（Harary）+ WW（超-Wiener）—— 首组基于 BFS 全源最短路径的距离类指数 |
| topo8 | `graph topo8` / `gtopo8` | V3.19 | ECI + 直径 + 半径 + 平均离心率 —— 基于离心率的指数族 |
| topo9 | `graph topo9` / `gtopo9` | V3.20 | W_S（Schultz MTI）+ W_G（Gutman 指数）+ CξE（连通离心指数）—— 度数-距离混合指数 |
| topo10 | `graph topo10` / `gtopo10` | V3.21 | Sz（Szeged）+ rSz（修订 Szeged）+ Mo（Mostar）—— 边划分距离指数 |
| topo11 | `graph topo11` / `gtopo11` | V3.22 | Balaban J + TI（传输不规则度）+ 顶点 PI —— 传输量类指数 |
| topo12 | `graph topo12` / `gtopo12` | V3.23 | M1\* / M2\* / M3\*（第一/第二/第三 Zagreb 离心率指数） |
| topo13 | `graph topo13` / `gtopo13` | V3.24 | TM1 / TM2 / GA_t（传输 Zagreb 指数） |
| topo14 | `graph topo14` / `gtopo14` | V3.25 | TE（总离心率）+ EDS（离心距离和）+ GEA（几何-算术离心率） |
| topo15 | `graph topo15` / `gtopo15` | V3.26 | LM1 / LM2 / LM3（跳跃 Zagreb 指数，二距离度数） |
| topo16 | `graph topo16` / `gtopo16` | V3.27 | R_{1/2}（乘积连通性）+ R_{-1}（倒数 Randić）+ Lz（兰州指数） |
| topo17 | `graph topo17` / `gtopo17` | V3.28 | M̄₁ / M̄₂ / F̄（Zagreb 补图指数） |
| topo18 | `graph topo18` / `gtopo18` | V3.29 | NM₁ / NM₂ / GA₂（邻域 Zagreb 指数） |
| topo19 | `graph topo19` / `gtopo19` | V3.30 | Λ（反向 Wiener）+ RCW + TW（终端 Wiener） |

每组命令另有若干语义别名（如 `graph zagreb` 亦可用 `zagreb index` 调用），详见对应硬化日志「Shell 接口」章节。累计至 V3.30，拓扑指数命令族共 19 组、57 个指数，加上 §13.1 的 Zagreb 四件套与谱/熵指标，宿主测试总数达约 **1273 个**（详细逐版本累计数见各硬化日志）。

## 十四、Neighborhood S-variant 拓扑指数命令族（`graph topo20` ~ `graph topo54`，V3.31 ~ V3.65 新增）

> 本节为 2026-07-19 本轮新增，补齐此前长期滞后的口径缺口：v1.6（2026-07-15）仅收录至 `graph topo19`（V3.30），其后 V3.31~V3.65 共 35 次硬化迭代新增的 `graph topo20`~`graph topo54` 命令族此前从未纳入本文档索引。

自 V3.31 起，拓扑指数命令族的定义方式发生系统性变化：不再直接对顶点度数 `d(v)` 取指数公式，而是先计算**邻域度和** `S(v) = Σ_{w∈N(v)} deg(w)`，再将 §13.2 中已有的经典/拓扑指数公式套用到 `S(v)` 上，得到该指数的 "Neighborhood S-variant"（邻域 S-变体）版本，指数名统一加前缀 `N`（如 M₁ → NM₁，Sombor → NSO）。这一模式自 topo18（NM₁/NM₂/GA₂，V3.29，见 §13.2）开始，至 topo54（V3.65）已连续扩展 37 组，是当前命令族中规模最大、仍在持续增长的部分。

每组命令固定新增 3 个指数，并配套一个独立的 `gos-graph-topoN-harness`（10 项测试，全部通过），命令别名统一遵循 `graph topoN` / `gtopoN` + 各指数的语义别名（如 `neighborhood sombor` / `gnso`）+ 组合别名（三个指数首字母缩写拼接）模式。完整公式推导、溢出安全性分析、S-正则图闭式公式与逐图交叉验证表，以对应 `doc/06_运维维护/hardening/HARDENING_LOG_*.md` 为唯一权威口径，本节仅做索引汇总，不重复维护公式细节。

| 组号 | 版本 | 新增指数（均为 S-变体） | 对应硬化日志 |
|---|---|---|---|
| topo20 | V3.31 | SO\*（修正 Sombor）+ RSO（约化 Sombor）+ rSO（简化 Sombor） | HARDENING_LOG_2026-07-15_V3.31.md |
| topo21 | V3.32 | ABC₄（4 代原子键连接性）+ NH（邻域调和）+ NSO（邻域 Sombor） | HARDENING_LOG_2026-07-15_V3.32.md |
| topo22 | V3.33 | NR（邻域 Randić）+ NF（邻域 Forgotten）+ NSC（邻域 Sum-Connectivity） | HARDENING_LOG_2026-07-15_V3.33.md |
| topo23 | V3.34 | NHM1（邻域超-第一 Zagreb）+ NSDD（邻域对称除法度）+ NM3（邻域不规则度绝对值） | HARDENING_LOG_2026-07-15_V3.34.md |
| topo24 | V3.35 | NISI（邻域反和入度）+ NAZI（邻域增广 Zagreb）+ NEM1（邻域重构第一 Zagreb） | HARDENING_LOG_2026-07-15_V3.35.md |
| topo25 | V3.36 | NHM2（邻域超-第二 Zagreb）+ NAG（邻域算术-几何比）+ NABS（邻域原子-键和连通性） | HARDENING_LOG_2026-07-15_V3.36.md |
| topo26 | V3.37 | NPC + NRM₂（邻域约化第二 Zagreb）+ NRSO（邻域约化 Sombor） | HARDENING_LOG_2026-07-16_V3.37.md |
| topo27 | V3.38 | NRR（邻域倒数 Randić）+ NSO\*（邻域修正 Sombor）+ NrSO（邻域简化 Sombor） | HARDENING_LOG_2026-07-16_V3.38.md |
| topo28 | V3.39 | NNI（邻域 Nirmala）+ NNMI（邻域修正 Nirmala）+ NSM1（邻域 M₁ 边形式） | HARDENING_LOG_2026-07-16_V3.39.md |
| topo29 | V3.40 | NZ₀（邻域零阶 Randić）+ NEM₂（邻域重构第二 Zagreb）+ NSe（邻域平方根顶点和） | HARDENING_LOG_2026-07-16_V3.40.md |
| topo30 | V3.41 | NVQ（邻域四次方顶点和）+ NRGS（邻域 3/2 阶广义 Randić）+ NHCS（邻域三次方边和） | HARDENING_LOG_2026-07-16_V3.41.md |
| topo31 | V3.42 | NSig（邻域 Sigma 不规则度）+ NHQS（邻域四次方边和）+ NPS（邻域五次方顶点和） | HARDENING_LOG_2026-07-16_V3.42.md |
| topo32 | V3.43 | NSH（邻域六次方顶点和）+ NHPS（邻域五次方边和）+ NWSO（邻域 Sombor 变体） | HARDENING_LOG_2026-07-16_V3.43.md |
| topo33 | V3.44 | NSHP（邻域七次方顶点和）+ NHSE（邻域六次方边和）+ NCSO（邻域三次 Sombor，α=3） | HARDENING_LOG_2026-07-16_V3.44.md |
| topo34 | V3.45 | NOC（邻域八次方顶点和）+ NHHS（邻域七次方边和）+ NFSO（邻域四次 Sombor，α=4） | HARDENING_LOG_2026-07-16_V3.45.md |
| topo35 | V3.46 | NNC（邻域九次方顶点和）+ NHOC（邻域八次方边和）+ NHSO（邻域六次 Sombor，α=6） | HARDENING_LOG_2026-07-16_V3.46.md |
| topo36 | V3.47 | NDC（邻域十次方顶点和）+ NHNC（邻域九次方边和）+ NOSO（邻域八次 Sombor，α=8） | HARDENING_LOG_2026-07-16_V3.47.md |
| topo37 | V3.48 | NUC（邻域十一次方顶点和）+ NHDC（邻域十次方边和）+ NTSO（邻域十次 Sombor，α=10） | HARDENING_LOG_2026-07-16_V3.48.md |
| topo38 | V3.49 | NDoC（邻域十二次方顶点和）+ NHUC（邻域十一次方边和）+ NDSO（邻域十二次 Sombor，α=12） | HARDENING_LOG_2026-07-16_V3.49.md |
| topo39 | V3.50 | NTC（邻域十三次方顶点和）+ NHDOC（邻域十二次方边和）+ NESO（邻域十四次 Sombor，α=14） | HARDENING_LOG_2026-07-16_V3.50.md |
| topo40 | V3.51 | NQTC（邻域十四次方顶点和）+ NHTC（邻域十三次方边和）+ NGSO（邻域十六次 Sombor，α=16） | HARDENING_LOG_2026-07-16_V3.51.md |
| topo41 | V3.52 | NPTC + NHQTC + NIOSO（邻域 Sombor 变体） | HARDENING_LOG_2026-07-16_V3.52.md |
| topo42 | V3.53 | NSTC + NHPTC + NJSO（邻域 Sombor 变体） | HARDENING_LOG_2026-07-16_V3.53.md |
| topo43 | V3.54 | NHEPTC + NHSTC + NKSO（邻域 Sombor 变体） | HARDENING_LOG_2026-07-16_V3.54.md |
| topo44 | V3.55 | NOCTC + NHOCTC + NLSO（邻域 Sombor 变体） | HARDENING_LOG_2026-07-16_V3.55.md |
| topo45 | V3.56 | NNONTC + NHNONTC + NMSO（邻域 Sombor 变体） | HARDENING_LOG_2026-07-16_V3.56.md |
| topo46 | V3.57 | NEICTC + NHEICTC + NNSO（邻域 Sombor 变体） | HARDENING_LOG_2026-07-16_V3.57.md |
| topo47 | V3.58 | NHENTC + NHHENTC + NPSO（邻域 Sombor 变体） | HARDENING_LOG_2026-07-16_V3.58.md |
| topo48 | V3.59 | NDOCTC + NHDOCTC + NQSO（邻域 Sombor 变体） | HARDENING_LOG_2026-07-17_V3.59.md |
| topo49 | V3.60 | NTRICTC（S-23 次方顶点和）+ NHTRICTC（S-22 次方边和）+ NRSO（邻域 Sombor，α=34） | HARDENING_LOG_2026-07-17_V3.60.md |
| topo50 | V3.61 | NTETRTC + NHTETRTC + NSSO（邻域 Sombor 变体） | HARDENING_LOG_2026-07-17_V3.61.md |
| topo51 | V3.62 | NPENTTC + NHPENTTC + NUSO（邻域 Sombor 变体） | HARDENING_LOG_2026-07-17_V3.62.md |
| topo52 | V3.63 | NHEXATC（S-26 次方顶点和）+ NHHEXATC（S-25 次方边和）+ NVSO（邻域 Sombor，α=40） | HARDENING_LOG_2026-07-17_V3.63.md |
| topo53 | V3.64 | NHEPTATC + NHHEPTATC + NXSO（邻域 Sombor 变体） | HARDENING_LOG_2026-07-17_V3.64.md |
| topo54 | V3.65 | NOCTATC + NHOCTATC + NYSO（邻域 Sombor 变体） | HARDENING_LOG_2026-07-17_V3.65.md |

累计至 V3.65，Neighborhood S-variant 拓扑指数命令族（topo20~topo54）共 35 组、105 个指数；加上 §13.2 的 topo1~topo19（19 组、57 个指数）与 §13.1 的 Zagreb 四件套/谱/熵指标，`graph topo*` 系列合计 54 组、约 165 个拓扑指数，VectorAddress L4 命名空间占用 88~142（`graph-topo`~`graph-topo54`）。宿主测试总数随本系列持续增长，最新累计数见 [README.md](../README.md) 06 · 运维维护表格与最新硬化日志。

**注**：部分指数缩写在不同组号间重复出现（如 topo26 与 topo49 均含 "NRSO"，topo41~topo54 多组共用 "N*SO" Sombor 变体命名模式），系源代码命名空间本身的既成事实（字母序列受限产生的缩写复用），本文档如实记录，不做归并或改名。

## 十五、Neighborhood S-variant 拓扑指数命令族续篇（`graph topo55` ~ `graph topo115`，V3.66 ~ V3.126 新增）

> 本节为 2026-07-21 本轮新增，2026-07-29 本轮扩展（v1.9→v1.10）。承接 §十四（v1.7 收录至 `graph topo54`/V3.65）。V3.66~V3.126 共 61 次硬化迭代（其中 V3.66、V3.102、V3.110、V3.111、V3.113 五版本文件缺失，详见下方说明），延续 §十四 的 Neighborhood S-variant 模式，将顶点/边幂次系列由 30 次幂推进至 89 次幂，Sombor 变体 α 由 46 推进至 166。NB 系列命名法沿用各硬化日志自述的字母/双字母标识（本文档仅逐条转录，不代为归纳统一规则），topo94~topo109 延续单段字母 K~Z，topo110 起硬化日志自述切换为 "AA"、单字母 "B"、双字母重复 "CC"/"DD"/"EE"/"FF" 等不完全规律的标识（详见下表逐条备注），如实转录，不做规则性总结或猜测性修正。

**顶点/边幂次系列命名节点**：topo55~65 为 "triacontic"（30余次幂）尾段，topo66~75 为 "tetracontic"（40余次幂）段，topo76~85 为 "pentacontic"（50余次幂）段，topo86~93 为 "hexacontic"（60余次幂）段首8个。每组命令固定新增 3 个指数，配套独立 `gos-graph-topoN-harness`（10 项测试）。命令别名、精确公式推导、S-正则图闭式公式与逐图交叉验证表，以对应 `doc/06_运维维护/hardening/HARDENING_LOG_*.md` 为唯一权威口径，本节仅做索引汇总。

| 组号 | 版本 | 新增指数（均为 S-变体） | 对应硬化日志 |
|---|---|---|---|
| topo55 | V3.66 | NNONATC（S²⁹顶点和）+ NHNONATC（边和）+ NZSO（α=46） | *文件缺失，仅见于 V3.67 等后续引用* |
| topo56 | V3.67 | NTRIACTC（S³⁰）+ NHTRIACTC（S²⁹边和）+ NASO（α=48，第2轮字母复用起点） | HARDENING_LOG_2026-07-19_V3.67.md |
| topo57 | V3.68 | NHENTRIACTC（S³¹）+ NHHENTRIACTC（S³⁰边和）+ NBSO（α=50） | HARDENING_LOG_2026-07-19_V3.68.md |
| topo58 | V3.69 | NDOTRIACTC（S³²）+ NHDOTRIACTC（S³¹边和）+ NAASO（α=52，双字母序列起点） | HARDENING_LOG_2026-07-19_V3.69.md |
| topo59 | V3.70 | NTRITRIACTC（S³³）+ NHTRITRIACTC（S³²边和）+ NABSO（α=54） | HARDENING_LOG_2026-07-19_V3.70.md |
| topo60 | V3.71 | NTETRTRIACTC（S³⁴）+ NHTETRTRIACTC（S³³边和）+ NACSO（α=56） | HARDENING_LOG_2026-07-19_V3.71.md |
| topo61 | V3.72 | NPENTTRIACTC（S³⁵）+ NHPENTTRIACTC（S³⁴边和）+ NADSO（α=58） | HARDENING_LOG_2026-07-19_V3.72.md |
| topo62 | V3.73 | NHEXATRIACTC（S³⁶）+ NHHEXATRIACTC（S³⁵边和）+ NAESO（α=60） | HARDENING_LOG_2026-07-20_V3.73.md |
| topo63 | V3.74 | NHEPTATRIACTC（S³⁷）+ NHHEPTATRIACTC（S³⁶边和）+ NAFSO（α=62） | HARDENING_LOG_2026-07-20_V3.74.md |
| topo64 | V3.75 | NOCTATRIACTC（S³⁸）+ NHOCTATRIACTC（S³⁷边和）+ NAGSO（α=64） | HARDENING_LOG_2026-07-20_V3.75.md |
| topo65 | V3.76 | NNONATRIACTC（S³⁹）+ NHNONATRIACTC（S³⁸边和）+ NAHSO（α=66） | HARDENING_LOG_2026-07-20_V3.76.md |
| topo66 | V3.77 | NTETRAACTC（S⁴⁰）+ NHTETRAACTC（S³⁹边和）+ NAISO（α=68） | HARDENING_LOG_2026-07-20_V3.77.md |
| topo67 | V3.78 | NHENTETRAACTC（S⁴¹）+ NHHENTETRAACTC（S⁴⁰边和）+ NAJSO（α=70） | HARDENING_LOG_2026-07-20_V3.78.md |
| topo68 | V3.79 | NDOTETRAACTC（S⁴²）+ NHDOTETRAACTC（S⁴¹边和）+ NAKSO（α=72） | HARDENING_LOG_2026-07-20_V3.79.md |
| topo69 | V3.80 | NTRITETRAACTC（S⁴³）+ NHTRITETRAACTC（S⁴²边和）+ NALSO（α=74） | HARDENING_LOG_2026-07-20_V3.80.md |
| topo70 | V3.81 | NTETRATETRAACTC（S⁴⁴）+ NHTETRATETRAACTC（S⁴³边和）+ NAMSO（α=76） | HARDENING_LOG_2026-07-20_V3.81.md |
| topo71 | V3.82 | NPENTETRAACTC（S⁴⁵）+ NHPENTETRAACTC（S⁴⁴边和）+ NANSO（α=78） | HARDENING_LOG_2026-07-20_V3.82.md |
| topo72 | V3.83 | NHEXTETRAACTC（S⁴⁶）+ NHHEXTETRAACTC（S⁴⁵边和）+ NAOSO（α=80） | HARDENING_LOG_2026-07-20_V3.83.md |
| topo73 | V3.84 | NHEPTETRAACTC（S⁴⁷）+ NHHEPTETRAACTC（S⁴⁶边和）+ NAPSO（α=82） | HARDENING_LOG_2026-07-20_V3.84.md |
| topo74 | V3.85 | NOCTOTETRAACTC（S⁴⁸）+ NHOCTOTETRAACTC（S⁴⁷边和）+ NAQSO（α=84） | HARDENING_LOG_2026-07-20_V3.85.md |
| topo75 | V3.86 | NNONATETRAACTC（S⁴⁹）+ NHNONATETRAACTC（S⁴⁸边和）+ NARSO（α=86） | HARDENING_LOG_2026-07-20_V3.86.md |
| topo76 | V3.87 | NPENTAACTC（S⁵⁰，pentacontic 系列首个）+ NHPENTAACTC（S⁴⁹边和）+ NASSO（α=88） | HARDENING_LOG_2026-07-20_V3.87.md |
| topo77 | V3.88 | NHENPENTAACTC（S⁵¹）+ NHHENPENTAACTC（S⁵⁰边和）+ NATSO（α=90） | HARDENING_LOG_2026-07-20_V3.88.md |
| topo78 | V3.89 | NDOPENTAACTC（S⁵²）+ NHDOPENTAACTC（S⁵¹边和）+ NAUSO（α=92） | HARDENING_LOG_2026-07-20_V3.89.md |
| topo79 | V3.90 | NTRIPENTAACTC（S⁵³）+ NHTRIPENTAACTC（S⁵²边和）+ NAVSO（α=94） | HARDENING_LOG_2026-07-20_V3.90.md |
| topo80 | V3.91 | NTETRAPENTAACTC（S⁵⁴）+ NHTETRAPENTAACTC（S⁵³边和）+ NAWSO（α=96） | HARDENING_LOG_2026-07-20_V3.91.md |
| topo81 | V3.92 | NPENTAPENTAACTC（S⁵⁵）+ NHPENTAPENTAACTC（S⁵⁴边和）+ NAXSO（α=98） | HARDENING_LOG_2026-07-20_V3.92.md |
| topo82 | V3.93 | NHEXPENTAACTC（S⁵⁶）+ NHHEXPENTAACTC（S⁵⁵边和）+ NAYSO（α=100，Centyl Sombor） | HARDENING_LOG_2026-07-20_V3.93.md |
| topo83 | V3.94 | NHEPTPENTAACTC（S⁵⁷）+ NHHEPTPENTAACTC（S⁵⁶边和）+ NAZSO（α=102，第3轮双字母收官） | HARDENING_LOG_2026-07-20_V3.94.md |
| topo84 | V3.95 | NOCTOPENTAACTC（S⁵⁸）+ NHOCTOPENTAACTC（S⁵⁷边和）+ NBASO（α=104，第4轮双字母 "B" 段起点） | HARDENING_LOG_2026-07-20_V3.95.md |
| topo85 | V3.96 | NNONAPENTAACTC（S⁵⁹，pentacontic 系列收官）+ NHNONAPENTAACTC（S⁵⁸边和）+ NBBSO（α=106） | HARDENING_LOG_2026-07-21_V3.96.md |
| topo86 | V3.97 | NHEXAACTC（S⁶⁰，hexacontic 系列首个）+ NHHEXAACTC（S⁵⁹边和）+ NBCSO（α=108） | HARDENING_LOG_2026-07-21_V3.97.md |
| topo87 | V3.98 | NHEXAENACTC（S⁶¹）+ NHHEXAENACTC（S⁶⁰边和）+ NBDSO（α=110） | HARDENING_LOG_2026-07-21_V3.98.md |
| topo88 | V3.99 | NHEXADYACTC（S⁶²）+ NHHEXADYACTC（S⁶¹边和）+ NBESO（α=112） | HARDENING_LOG_2026-07-21_V3.99.md |
| topo89 | V3.100 | NHEXATRIACTC（S⁶³，首个三位数版本号里程碑）+ NHHEXATRIACTC（S⁶²边和）+ NBFSO（α=114） | HARDENING_LOG_2026-07-21_V3.100.md |
| topo90 | V3.101 | NHEXATETRAACTC（S⁶⁴）+ NHHEXATETRAACTC（S⁶³边和）+ NBGSO（α=116） | HARDENING_LOG_2026-07-21_V3.101.md |
| topo91 | V3.102 | NHEXAPENTAACTC（S⁶⁵）+ NHHEXAPENTAACTC（S⁶⁴边和）+ NBHSO（α=118） | *文件缺失，仅见于 V3.101/V3.103/V3.104 交叉引用* |
| topo92 | V3.103 | NHEXAHEXAACTC（S⁶⁶）+ NHHEXAHEXAACTC（S⁶⁵边和）+ NBISOS（α=120） | HARDENING_LOG_2026-07-21_V3.103.md |
| topo93 | V3.104 | NHEXAHEPTACTC（S⁶⁷）+ NHHEXAHEPTACTC（S⁶⁶边和）+ NBJSO（α=122） | HARDENING_LOG_2026-07-21_V3.104.md |
| topo94 | V3.105 | NHEXAOCTACTC（S⁶⁸，hexacontic 系列第9个）+ NHHEXAOCTACTC（边和）+ NBKSO（α=124） | HARDENING_LOG_2026-07-21_V3.105.md |
| topo95 | V3.106 | NHEXAENNACTC（S⁶⁹，hexacontic 系列收官/第10个）+ NHHEXAENNACTC（边和）+ NBLSO（α=126） | HARDENING_LOG_2026-07-21_V3.106.md |
| topo96 | V3.107 | NHEPTAACTC（S⁷⁰，heptacontic 系列首个）+ NHHEPTAACTC（边和）+ NBMSO（α=128） | HARDENING_LOG_2026-07-21_V3.107.md |
| topo97 | V3.108 | NHEPTAENACTC（S⁷¹）+ NHHEPTAENACTC（边和）+ NBNSO（α=130） | HARDENING_LOG_2026-07-21_V3.108.md |
| topo98 | V3.109 | NHEPTADIACTC（S⁷²）+ NHHEPTADIACTC（边和）+ NBOSO（α=132） | HARDENING_LOG_2026-07-21_V3.109.md |
| topo99 | V3.110 | NHEPTATRIACTC（S⁷³）+ NHHEPTATRIACTC（边和）+ NBPSO（α=134） | *文件缺失，仅见于 git 提交 `1d074aa feat(v3.110)`* |
| topo100 | V3.111 | NHEPTATETRAACTC（S⁷⁴）+ NHHEPTATETRAACTC（边和）+ NBQSO（α=136） | *文件缺失，仅见于 git 提交 `9784244 feat(v3.111)`* |
| topo101 | V3.112 | NHEPTAPENTACTC（S⁷⁵）+ NHHEPTAPENTACTC（边和）+ NBRSO（α=138） | HARDENING_LOG_2026-07-22_V3.112.md |
| topo102 | V3.113 | NHEPTAHEXAACTC（S⁷⁶）+ NHHEPTAHEXAACTC（边和）+ NBSSO（α=140） | *文件缺失，仅见于 git 提交 `90b891e feat(v3.113)`* |
| topo103 | V3.114 | NHEPTAHEPTAACTC（S⁷⁷）+ NHHEPTAHEPTAACTC（边和）+ NBTSO（α=142） | HARDENING_LOG_2026-07-22_V3.114.md |
| topo104 | V3.115 | NHEPTAOCTAACTC（S⁷⁸）+ NHHEPTAOCTAACTC（边和）+ NBUSO（α=144） | HARDENING_LOG_2026-07-22_V3.115.md |
| topo105 | V3.116 | NHEPTAENNACTC（S⁷⁹，heptacontic 系列收官/第10个）+ NHHEPTAENNACTC（边和）+ NBVSO（α=146） | HARDENING_LOG_2026-07-22_V3.116.md |
| topo106 | V3.117 | NOCTAACTC（S⁸⁰，octacontic 系列首个）+ NHOCTAACTC（边和）+ NBWSO（α=148） | HARDENING_LOG_2026-07-22_V3.117.md |
| topo107 | V3.118 | NOCTAMONOACTC（S⁸¹）+ NHOCTAMONOACTC（边和）+ NBXSO（α=150） | HARDENING_LOG_2026-07-28_V3.118.md |
| topo108 | V3.119 | NOCTADIACTC（S⁸²）+ NHOCTADIACTC（边和）+ NBYSO（α=152） | HARDENING_LOG_2026-07-29_V3.119.md |
| topo109 | V3.120 | NOCTATRIACTC（S⁸³）+ NHOCTATRIACTC（边和）+ NBZSO（α=154） | HARDENING_LOG_2026-07-29_V3.120.md |
| topo110 | V3.121 | NOCTATETRAACTC（S⁸⁴）+ NHOCTATETRAACTC（边和）+ NBAASO（α=156，硬化日志自述标识切换为 "AA"） | HARDENING_LOG_2026-07-29_V3.121.md |
| topo111 | V3.122 | NOCTAPENTACTC（S⁸⁵）+ NHOCTAPENTACTC（边和）+ NBBSO（α=158，硬化日志自述标识为单字母 "B"） | HARDENING_LOG_2026-07-29_V3.122.md |
| topo112 | V3.123 | NOCTAHEXACTC（S⁸⁶）+ NHOCTAHEXACTC（边和）+ NBCCSO（α=160，硬化日志自述标识为重复字母 "CC"） | HARDENING_LOG_2026-07-29_V3.123.md |
| topo113 | V3.124 | NOCTAHEPTACTC（S⁸⁷）+ NHOCTAHEPTACTC（边和）+ NBDDSO（α=162，"DD"） | HARDENING_LOG_2026-07-29_V3.124.md |
| topo114 | V3.125 | NOCTAOCTACTC（S⁸⁸）+ NHOCTAOCTACTC（边和）+ NBEESO（α=164，"EE"） | HARDENING_LOG_2026-07-29_V3.125.md |
| topo115 | V3.126 | NOCTAENNACTC（S⁸⁹，八旬系列第10个/最终）+ NHOCTAENNACTC（边和）+ NBFFSO（α=166，"FF"，第32个NB组） | HARDENING_LOG_2026-07-29_V3.126.md |

累计至 V3.126，Neighborhood S-variant 拓扑指数命令族（topo20~topo115）共 96 组、288 个指数；加上 §13.2 的 topo1~topo19（19 组、57 个指数）与 §13.1 的 Zagreb 四件套/谱/熵指标，`graph topo*` 系列合计 115 组、约 345 个拓扑指数，VectorAddress L4 命名空间占用 88~202（`graph-topo`~`graph-topo115`）。宿主测试总数 2231（topo115 harness 10 项全绿），最新累计数见 [README.md](../README.md) 06 · 运维维护表格与最新硬化日志。

**已知缺口（5 项，均已用 `git log --all --oneline` 核实为文档侧遗漏，非代码或测试缺失）**：

| 版本 | topo | 对应 git 提交（`feat`，代码 + 测试均已合入） | docs 提交 |
|---|---|---|---|
| V3.66 | topo55 | `755aa5e feat(v3.66): NNONATC + NHNONATC + NZSO ... (10 tests)` | 未找到 |
| V3.102 | topo91 | `e7d5c74 feat(v3.102): NHEXAPENTACTC + NHHEXAPENTACTC + NBHSO ... (10 tests)` | 未找到 |
| V3.110 | topo99 | `1d074aa feat(v3.110): NHEPTATRIACTC + NHHEPTATRIACTC + NBPSO ... (10 新测试)` | 未找到 |
| V3.111 | topo100 | `9784244 feat(v3.111): NHEPTATETRAACTC + NHHEPTATETRAACTC + NBQSO ... (10 新测试)` | 未找到 |
| V3.113 | topo102 | `90b891e feat(v3.113): NHEPTAHEXAACTC + NHHEPTAHEXAACTC + NBSSO ... (10 新测试)` | 未找到 |

上一轮（2026-07-21）曾将 V3.66/V3.102 标记为「待核实是否为归档遗漏、误删除，或版本号本身从未生成对应日志」；本轮已用 `git log --all --oneline --diff-filter=A -- '*V3.66*' '*V3.102*'` 等命令核实：五个版本号均确有对应的 `feat(vX.XXX)` 提交（代码与 harness 测试均已合入主线并通过），但均从未出现对应的 `docs(vX.XXX): 强化日志归档` 提交——即硬化日志 markdown 文件在这五个版本上从未被创建过，属于文档编写环节的一次性遗漏，而非归档丢失、误删除或代码缺失。该待核实项自本轮起解除阻塞。

**已知口径不一致（2 项，均如实保留原文数值，未擅自修正）**：

1. V3.100（topo89）硬化日志自述"宿主测试总数 1963（此前 1953）"，但 V3.99（topo88）已自述其累计总数为 1963；两者不能同时成立（差 10 项，疑似 V3.100 落款时误抄 V3.99 的数值而非累计 +10）。建议以逐版本 `cargo test` 实际结果为准核实真实累计数。
2. **（本轮新发现）** V3.117（topo106）硬化日志摘要与「测试结果」两处均自述「宿主测试套件累计：2131 → 2141」，与 V3.116（topo105）自述的累计终值完全相同（应为 2141 → 2151）；经核对 V3.118（topo107）摘要自述起始基数为 2141（即以 2151 为其累计终值的前提反推，V3.118 = 2141+10=2151 成立），可判定 V3.117 落款时误将 V3.116 的数值原样复制、忘记递增，但该错误未影响后续版本的真实累计计数（V3.118 起已自行恢复正确基数）。本文档如实保留 V3.117 原文数值，不擅自修正。

## 十六、输出说明

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

## 十七、最短示例

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
