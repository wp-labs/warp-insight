import { Routes, Route, Navigate } from "react-router-dom";

import { AppLayout } from "./components/AppLayout";
import { SubsystemAdminHomePage } from "./components/SubsystemAdminHomePage";
import { SubsystemAgentControlCenterPage } from "./components/SubsystemAgentControlCenterPage";
import { SubsystemAgentInstallPage } from "./components/SubsystemAgentInstallPage";
import { SubsystemGatewayInitializePage } from "./components/SubsystemGatewayInitializePage";
import { SubsystemAgentHostMetricsPage } from "./components/SubsystemAgentHostMetricsPage";
import { SubsystemAgentHostListPage } from "./components/SubsystemAgentHostListPage";

export function App() {
  return (
    <Routes>
      <Route element={<AppLayout />}>
        <Route path="/" element={<SubsystemAdminHomePage />} />
        <Route path="/control" element={<SubsystemAgentControlCenterPage />} />
        <Route path="/install" element={<SubsystemAgentInstallPage />} />
        <Route path="/init" element={<SubsystemGatewayInitializePage />} />
        <Route path="/hosts" element={<SubsystemAgentHostListPage />} />
        <Route
          path="/agents/:agentId/metrics"
          element={<SubsystemAgentHostMetricsPage />}
        />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  );
}
