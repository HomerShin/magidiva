import { Component, type ReactNode } from "react";

interface Props { children: ReactNode }
interface State { hasError: boolean; error: Error | null }

export default class ErrorBoundary extends Component<Props, State> {
  state: State = { hasError: false, error: null };

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: { componentStack?: string }) {
    // Attempt to write to a file the user can find — fallback alert if fs fails
    try {
      const line = `[MagiDiva Crash] ${new Date().toISOString()}\n${error.message}\n\n${error.stack ?? "(no stack)"}\n\nComponent stack:\n${info.componentStack ?? "n/a"}\n\n---\n`;
      // Tauri v2 fs — try a couple of known writable paths
      const tryLog = async () => {
        try {
          const { writeTextFile, BaseDirectory } = await import("@tauri-apps/plugin-fs");
          await writeTextFile("magidiva-crash.log", line, {
            baseDir: BaseDirectory.AppData,
            append: true,
          });
        } catch { /* ignore */ }
      };
      tryLog();
    } catch { /* ignore */ }
    console.error("MagiDiva crash:", error, info);
  }

  render() {
    if (this.state.hasError) {
      const err = this.state.error;
      return (
        <div style={{
          padding: 40, maxWidth: 640, margin: "60px auto",
          background: "#1e1e24", border: "1px solid #444", borderRadius: 12,
          color: "#fff", fontFamily: "system-ui, sans-serif",
        }}>
          <h2 style={{ color: "#ff6b6b", marginBottom: 8 }}>😵 Application Crash</h2>
          <p style={{ opacity: 0.7, marginBottom: 20 }}>
            Something inside the renderer failed. Tell the developer what you were doing
            when this appeared. Error is also written to
            <code style={{ background: "#000", padding: "2px 6px", borderRadius: 4 }}>
              AppData/magidiva-crash.log
            </code>
          </p>
          {err && (
            <pre style={{
              background: "#000", padding: 16, borderRadius: 8,
              fontSize: 12, overflow: "auto", whiteSpace: "pre-wrap", wordBreak: "break-all",
            }}>
              {err.message}{"\n\n"}{err.stack}
            </pre>
          )}
          <button
            onClick={() => window.location.reload()}
            style={{
              marginTop: 20, padding: "8px 20px", background: "#4dabf7",
              border: "none", borderRadius: 6, color: "#fff", cursor: "pointer",
              fontSize: 14, fontWeight: 600,
            }}
          >
            Reload
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
