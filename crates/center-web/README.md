# center-web — WarpInsightCenter 全局控制中心前端

WarpInsightCenter 子系统的浏览器端 WEB 前端（Jumo `module<ui> InsightCenterWeb`，`target<site,web> center-web`）。承载网关列表/实例、初始配置、版本发布与升级计划页面。

## 页面

| 路由 | 页面 | 用例 |
|---|---|---|
| `/` | 网关列表 | ViewGatewayStatus |
| `/instance` | 网关实例 | CreateGatewayInstance / BindGatewayCustomer |
| `/config` | 初始配置 | GetInitialConfig |
| `/release` | 版本发布 | PublishWistAgentd / PublishWarpGateWay |
| `/upgrade-plan` | 升级计划 | CreateUpgradePlan / ApproveUpgradePlan |

## 快速开始

```bash
npm install
npm run dev      # http://localhost:5173
npm run build    # tsc -b && vite build
```

## 数据来源

- 页面通过 `AdminFacingInterface` 语义的 `/api/v1/admin/...` 管理接口消费数据。
- 后端 `wist-center`（原 warp-insight-center）尚未实现 HTTP 管理接口时，请求失败自动回退到内置
  example 数据（`source: "example"`），页面独立可渲染；接入真实后端后自动切换为
  `"real"`。填入 Admin Token 后开启 5s 轮询刷新。

## 生成管线（可复现）

```bash
npx tsx generate.ts jumo-ui-model.json .   # 从 jumo-ui-model.json 生成骨架
```

`jumo-ui-model.json` 描述 UI 结构（对应 `WarpInsightCenter/layout.mju` 的区域与类型），
生成器产出 `src/types`、`src/store` 与组件骨架；页面设计在生成骨架上手工精修。
