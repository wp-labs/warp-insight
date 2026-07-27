# warp-insight 设计文档索引

`doc/design` 目录按主题拆分为以下子目录：

- `foundation/`
  总体目标、总体架构、术语、路线图、实现 backlog、非功能目标、安全模型、错误处理体系、参考资料
- `execution/`
  动作 DSL、`ActionPlan IR`、执行 schema、opcode schema、`run.gxl` 相关文档
- `edge/`
  `warp-insightd` / `warp-insight-exec` / 本地状态 / 故障处理 / 资源发现 / logs file state / 配置 / capability / 自观测
- `center/`
  控制中心、控制平面、管理平台容器 bootstrap、Gateway 协议、计划投递、discovery 同步、主机库存与运行态、软件归一化、公开漏洞源接入、图谱关系与结果回报 schema
- `telemetry/`
  metrics 集成、logs file input、discovery、resource mapping、uplink 设计与 Batch A 规格

建议阅读顺序：

1. [foundation/target.md](./foundation/target.md)
2. [foundation/architecture.md](./foundation/architecture.md)
3. [foundation/security-model.md](./foundation/security-model.md)
4. [foundation/error-handling-system.md](./foundation/error-handling-system.md)
5. [execution/action-plan-ir.md](./execution/action-plan-ir.md)
6. [edge/agentd-architecture.md](./edge/agentd-architecture.md)
7. [edge/agentd-failure-handling.md](./edge/agentd-failure-handling.md)
8. [edge/resource-discovery-runtime.md](./edge/resource-discovery-runtime.md)
9. [edge/discovery-runtime-current-state.md](./edge/discovery-runtime-current-state.md)
10. [edge/discovery-output-examples-current.md](./edge/discovery-output-examples-current.md)
11. [edge/discovery-vs-resource-state-current.md](./edge/discovery-vs-resource-state-current.md)
12. [center/control-center-architecture.md](./center/control-center-architecture.md)
13. [center/identity-enrollment-protocol.md](./center/identity-enrollment-protocol.md)
14. [center/management-platform-bootstrap.md](./center/management-platform-bootstrap.md)
15. [center/report-discovery-snapshot-schema.md](./center/report-discovery-snapshot-schema.md)
16. [center/discovery-sync-protocol.md](./center/discovery-sync-protocol.md)
17. [center/models/software-normalization-and-vuln-enrichment.md](./center/models/software-normalization-and-vuln-enrichment.md)
18. [center/models/public-vulnerability-source-ingestion.md](./center/models/public-vulnerability-source-ingestion.md)
19. [center/models/host-inventory-and-runtime-state.md](./center/models/host-inventory-and-runtime-state.md)
20. [center/models/host-inventory-and-runtime-state-schema.md](./center/models/host-inventory-and-runtime-state-schema.md)
21. [center/models/host-inventory-and-runtime-state-storage.md](./center/models/host-inventory-and-runtime-state-storage.md)
22. [center/models/host-responsibility-and-maintainer-model.md](./center/models/host-responsibility-and-maintainer-model.md)
23. [center/models/host-responsibility-sync-from-external-systems.md](./center/models/host-responsibility-sync-from-external-systems.md)
24. [center/models/host-pod-network-topology-model.md](./center/models/host-pod-network-topology-model.md)
25. [center/models/host-process-software-vulnerability-graph.md](./center/models/host-process-software-vulnerability-graph.md)
26. [center/models/business-system-service-topology-model.md](./center/models/business-system-service-topology-model.md)
27. [telemetry/metrics-integration-roadmap.md](./telemetry/metrics-integration-roadmap.md)
28. [telemetry/telemetry-uplink-and-warp-parse.md](./telemetry/telemetry-uplink-and-warp-parse.md)
29. [telemetry/log-file-input-spec.md](./telemetry/log-file-input-spec.md)
30. [edge/log-file-state-schema.md](./edge/log-file-state-schema.md)
31. [foundation/implementation-backlog.md](./foundation/implementation-backlog.md)
