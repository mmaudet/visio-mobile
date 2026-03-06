import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./App.css";

// Load Tauri mock when running in browser (npm run dev without Tauri)
const win = window as Window & { __TAURI_INTERNALS__?: unknown };
if (!win.__TAURI_INTERNALS__) {
  import("./tauri-mock").then(({ setupTauriMock }) => {
    setupTauriMock();
    renderApp();
  });
} else {
  renderApp();
}

function renderApp() {
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App />
    </StrictMode>
  );
}
