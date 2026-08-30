import {HashRouter, Routes, Route, useNavigate} from "react-router-dom";
import HomePage from "./pages/HomePage.tsx";
import SettingsPage from "./pages/SettingsPage.tsx";
import {useEffect} from "react";

// Helper component to handle incoming window messages
function ExtensionMessageListener() {
  const navigate = useNavigate();

  useEffect(() => {
    const handleMessage = (event: MessageEvent) => {
      const message = event.data;
      if (message.command === 'navigate') {
        navigate(message.path);
      }
    };

    window.addEventListener('message', handleMessage);
    return () => window.removeEventListener('message', handleMessage);
  }, [navigate]);

  return null;
}

export default function App() {
  return (
    <HashRouter>
      <ExtensionMessageListener />
      <Routes>
        {/* The main Dockview workspace dashboard */}
        <Route path="/" index element={<HomePage />} />

        {/* Optional: A separate dedicated route if opened via a distinct extension command */}
        <Route path="/settings" element={<SettingsPage />} />
      </Routes>
    </HashRouter>
  );
}