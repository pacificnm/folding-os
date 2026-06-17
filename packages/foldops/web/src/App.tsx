import { Navigate, Route, Routes } from "react-router-dom";
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
      <Route path="/deploy" element={<Deploy />} />
      <Route path="/alerts" element={<AlertHistory />} />
      <Route path="/machine/:hostname" element={<MachineDetail />} />
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}
