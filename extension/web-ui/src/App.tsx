import { HashRouter, Routes, Route, useNavigate } from "react-router-dom";
import { useEffect } from "react";
import HomePage from "./pages/HomePage";
import DBPage from "./pages/DBPage";

function ExtensionMessageListener() {
  const navigate = useNavigate();

  useEffect(() => {
    const handleMessage = (event: MessageEvent) => {
      const message = event.data;
      if (message.command === 'navigate' && message.path) {
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
        <Route path="/" index element={<HomePage />} />
        <Route path="/db" element={<DBPage />} />
      </Routes>
    </HashRouter>
  );
}