import { Navigate, Route, Routes } from "react-router-dom";
import { AdminLayout } from "./pages/admin/AdminLayout";
import { AdminIndex } from "./pages/admin/AdminIndex";
import { AdminFoldingHome } from "./pages/admin/AdminFoldingHome";
import { AdminLogs } from "./pages/admin/AdminLogs";
import { AdminMachines } from "./pages/admin/AdminMachines";
import { AdminRecovery } from "./pages/admin/AdminRecovery";
import { AdminServices } from "./pages/admin/AdminServices";
import { AdminSoftwareUpdates } from "./pages/admin/AdminSoftwareUpdates";
import { Dashboard } from "./pages/Dashboard";
import { KioskHome } from "./pages/KioskHome";
import { AlertHistory } from "./pages/AlertHistory";
import { Deploy } from "./pages/Deploy";
import { MachineDetail } from "./pages/MachineDetail";

export default function App() {
  return (
    <Routes>
      <Route path="/" element={<KioskHome />} />
      <Route path="/dashboard" element={<Dashboard />} />
      <Route path="/admin" element={<AdminLayout />}>
        <Route index element={<AdminIndex />} />
        <Route path="machines" element={<AdminMachines />} />
        <Route path="folding" element={<AdminFoldingHome />} />
        <Route path="logs" element={<AdminLogs />} />
        <Route path="software" element={<AdminSoftwareUpdates />} />
        <Route path="services" element={<AdminServices />} />
        <Route path="recovery" element={<AdminRecovery />} />
      </Route>
      <Route path="/deploy" element={<Deploy />} />
      <Route path="/alerts" element={<AlertHistory />} />
      <Route path="/machine/:hostname" element={<MachineDetail />} />
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}
