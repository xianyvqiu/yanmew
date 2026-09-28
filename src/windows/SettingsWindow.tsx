import { useCurrentContext } from "../hooks/useCurrentContext";
import SettingsPage from "../pages/SettingsPage";
import { usePetAppearance } from "./usePetAppearance";

export default function SettingsWindow() {
  const { snapshot, loading, error, refresh } = useCurrentContext();
  const appearance = usePetAppearance();
  return <SettingsPage snapshot={snapshot} loading={(loading && !snapshot) || !appearance.ready}
    error={error || appearance.error} onRefresh={refresh} />;
}
