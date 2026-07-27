import { Routes, Route, Navigate } from "react-router-dom";

import { SubsystemAdminHomePage } from "./components/SubsystemAdminHomePage";
import { SubsystemAgentControlCenterPage } from "./components/SubsystemAgentControlCenterPage";
import { SubsystemAgentInstallPage } from "./components/SubsystemAgentInstallPage";

export function App() {
  return (
    <Routes>
      <Route path="/" element={<SubsystemAdminHomePage />} />
      <Route path="/control" element={<SubsystemAgentControlCenterPage />} />
      <Route path="/install" element={<SubsystemAgentInstallPage />} />
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}
