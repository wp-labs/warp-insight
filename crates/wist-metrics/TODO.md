# wist-metrics TODO

本 crate 承载「非系统指标采集」：契约（`MetricProvider` trait + outcome/sample/target-entry + spec 表）
已下沉至此，`warp-agentd` 只做再导出。以下是 DB 指标采集（Postgres / MySQL）的剩余工作。

## 待落地

1. **加 `sqlx` 依赖（feature 门控）**
   - `sqlx` 用 `default-features = false` + `runtime-tokio-rustls` + `tls-rustls-ring`（或等价）驱动；
   - 拆 `postgres` / `mysql` 两个 feature，默认关闭，保证 `warp-agentd` 默认构建不碰 sqlx；
   - DB 驱动只在本 crate，不进 `warp-agentd` 依赖图。

2. **实现 `PostgresMetricsProvider` / `MysqlMetricsProvider`**
   - 采集 query 集参考 `pg_exporter`（`pg_stat_database` / `pg_stat_bgwriter` / `pg_stat_replication`
     / 连接数 / 长事务）与 `mariadb_exporter`（`SHOW GLOBAL STATUS` / `SHOW ENGINE INNODB STATUS` / 复制位点）；
   - provider 只产出 runtime fact（`fact_key → 值`），不做上报决策（W4 边界纪律）。

3. **spec 表加 `pg.*` / `mysql.*` 指标声明**
   - 与 provider 同处本 crate，保持「新增指标 = 加 spec + 加 provider」在 `wist-metrics` 一处完成；
   - 命名沿用现有风格（如 `postgres.connections.active`、`mysql.threads.connected`），
     待与 warp-insight 规范化命名对齐后定稿。

4. **映射层打通（`warp-agentd` 侧）**
   - `planner_bridge::build_collection_candidates`：`service_endpoint`（端口 5432/3306）→ db candidate；
   - `planner_candidates` 新增 postgres/mysql 候选文件路径；
   - `target_view` 加载列表、`daemon.rs` 候选过滤与存储、`runtime::providers()` 注册；
   - capability 声明经 `providers()` 自动带入，无需单独改 `capability_report`。

5. **异步/超时模型（与现有同步 `MetricProvider::collect` 衔接）**
   - 见下方「待决策 ②」。

## 待决策

1. **DSN 来源**：discovery 只给「端口 + 进程」，拿不到用户名/密码。
   - 方案 A：`warp-agentd` config 新增 `db_metrics` 段（每目标一个 DSN 或全局连接串）；
   - 方案 B：DSN 由 center 下发（计划内携带）——依赖 W4 capability 协商已就绪。
   - 当前 `warp-agentd` 无任何 dsn/database 配置，属全新添加。

2. **DB 采集的异步/超时模型**：`MetricProvider::collect` 是同步 `fn`，DB 查询是网络 I/O。
   - 倾向：同步 provider + 内部 `spawn_blocking` + 短超时（对 daemon tick 模型改动最小）；
   - 备选：给 provider 加异步采集路径（改动更大，需重排 `build_runtime_snapshot_from_view`）。
