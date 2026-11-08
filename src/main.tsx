import { createRoot } from "react-dom/client";
import App from "./App";
import ErrorBoundary from "./ErrorBoundary";

// Global error handlers — anything that blows up before React renders also gets caught
window.addEventListener("error", (ev) => {
  console.error("[window.onerror]", ev.message, ev.filename, ev.lineno);
  try {
    const body = document.body;
    if (body && !document.getElementById("__global_err__")) {
      const div = document.createElement("div");
      div.id = "__global_err__";
      div.style.cssText = "padding:40px;color:#fff;background:#1e1e24;font-family:system-ui;position:fixed;inset:0;overflow:auto";
      div.innerHTML = `<h2 style="color:#ff6b6b">😵 Startup Error</h2>
        <p style="opacity:.7">The app failed to load completely.</p>
        <pre style="background:#000;padding:16px;border-radius:8px;font-size:12px">${ev.message}\n\n${ev.filename}:${ev.lineno}:${ev.colno}\n\n${ev.error?.stack ?? ""}</pre>
        <button onclick="location.reload()" style="margin-top:20px;padding:8px 20px;background:#4dabf7;border:none;border-radius:6px;color:#fff;cursor:pointer;font-weight:600">Reload</button>`;
      body.appendChild(div);
    }
  } catch { /* ignore */ }
});

window.addEventListener("unhandledrejection", (ev) => {
  console.error("[unhandledrejection]", ev.reason);
});

createRoot(document.getElementById("root")!).render(
  <ErrorBoundary>
    <App />
  </ErrorBoundary>
);
