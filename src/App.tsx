import { useState, useCallback, useEffect, useRef } from "react";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import "./App.css";

interface FileItem {
  id: string;
  path: string;
  name: string;
  previewUrl: string;
  outputUrl?: string;
  status: "pending" | "processing" | "done" | "error";
  outputPath?: string;
  error?: string;
}
interface SizePreset { name: string; w: number; h: number; }
interface ModelPreset { name: string; inputSize: number; description: string; }
interface WmPosition { label: string; value: string; }
interface PhaseStarting { phase: "starting"; model: string }
interface PhaseDownloading { phase: "downloading"; model: string; percent: number }
interface PhaseInferring { phase: "inferring" }
interface PhaseSaving { phase: "saving" }
interface PhaseDone { phase: "done"; success: boolean; error?: string }
type Phase = PhaseStarting | PhaseDownloading | PhaseInferring | PhaseSaving | PhaseDone;

interface BatchProgress {
  current: number; total: number; input: string; phase: Phase;
}

function phaseLabel(p: Phase): string {
  switch (p.phase) {
    case "starting":    return `Loading model ${p.model}...`;
    case "downloading": return `Downloading ${p.model}... ${p.percent}%`;
    case "inferring":   return "Running AI background removal...";
    case "saving":      return "Saving output...";
    case "done":        return p.success ? "Done ✓" : `Failed: ${p.error ?? "unknown"}`;
  }
}
interface Toast { id: number; msg: string; type: "success" | "error" | "info"; }

function fileUrl(p: string) {
  // Tauri v2 asset protocol — properly escapes Windows paths
  try { return convertFileSrc(p); } catch {
    // Fallback for browser preview (non-Tauri context)
    return p.replace(/\\/g, "/").replace(/^([A-Za-z]:)/, "file:///$1");
  }
}

function App() {
  const [files, setFiles] = useState<FileItem[]>([]);
  const [outputDir, setOutputDir] = useState("");
  const [isProcessing, setIsProcessing] = useState(false);
  const [dragOver, setDragOver] = useState(false);
  const [whiteBg, setWhiteBg] = useState(true);
  const [shadow, setShadow] = useState(false);
  const [shadowStyle, setShadowStyle] = useState<"soft" | "medium" | "strong" | "offset">("medium");
  const [bgColor, setBgColor] = useState("#ffffff");
  const [outFormat, setOutFormat] = useState<"png" | "jpg" | "webp">("png");
  const [quality, setQuality] = useState(92);
  const [center, setCenter] = useState(true);
  const [presets, setPresets] = useState<SizePreset[]>([]);
  const [selectedPreset, setSelectedPreset] = useState("Shopify 2048x2048");
  const [models, setModels] = useState<ModelPreset[]>([]);
  const [selectedModel, setSelectedModel] = useState("birefnet-general-lite");
  const [progress, setProgress] = useState<{ current: number; total: number; phase?: Phase } | null>(null);
  const [wmPath, setWmPath] = useState<string>("");
  const [wmOpacity, setWmOpacity] = useState(0.7);
  const [wmPosition, setWmPosition] = useState("br");
  const [wmPositions, setWmPositions] = useState<WmPosition[]>([]);
  const [toasts, setToasts] = useState<Toast[]>([]);
  // 输出文件名固定（如 xxx_centered.png），URL 不变会被 WebView2 缓存，
  // 导致重新处理后预览还是旧图。每次批处理完成后自增，作为 URL 查询参数破缓存。
  const [previewNonce, setPreviewNonce] = useState(0);
  const [selectedFileId, setSelectedFileId] = useState<string | null>(null);
  const [isPro, setIsPro] = useState(false);
  const [appVersion, setAppVersion] = useState("0.0.0");
  const addPathsRef = useRef<(paths: string[]) => void>(() => {});
  const cancelledRef = useRef(false);

  const addToast = useCallback((msg: string, type: Toast["type"] = "info") => {
    const id = Date.now() + Math.random();
    setToasts((prev) => [...prev, { id, msg, type }]);
    setTimeout(() => setToasts((prev) => prev.filter((t) => t.id !== id)), 3500);
  }, []);

  useEffect(() => {
    // Feature flag check — drives Community vs Pro UI
    invoke<{ is_pro: boolean; version: string }>("app_info").then(
      (info) => { setIsPro(info.is_pro); setAppVersion(info.version); },
      () => {}
    );

    invoke<[string, number, number][]>("list_size_presets").then(
      (list) => setPresets(list.map(([name, w, h]) => ({ name, w, h }))),
      () => setPresets([
        { name: "Original", w: 0, h: 0 },
        { name: "Shopify 2048x2048", w: 2048, h: 2048 },
        { name: "Shopify 1024x1024", w: 1024, h: 1024 },
        { name: "Etsy 2000x2000", w: 2000, h: 2000 },
        { name: "Amazon Main 2000x2000", w: 2000, h: 2000 },
      ])
    );

    invoke<[string, string][]>("list_wm_positions").then(
      (list) => setWmPositions(list.map(([label, value]) => ({ label, value }))),
      () => setWmPositions([
        { label: "Bottom Right", value: "br" },
        { label: "Bottom Left", value: "bl" },
        { label: "Top Right", value: "tr" },
        { label: "Top Left", value: "tl" },
      ])
    );

    invoke<[string, number, string][]>("list_models").then(
      (list) => {
        setModels(list.map(([name, inputSize, description]) => ({ name, inputSize, description })));
        if (list.some(([n]) => n === selectedModel)) return;
        // Default to best quality that's practical
        setSelectedModel("birefnet-general-lite");
      },
      () => setModels([
        { name: "birefnet-general-lite", inputSize: 1024, description: "High accuracy, ~5s CPU" },
        { name: "u2net", inputSize: 320, description: "Fast baseline, ~2s CPU" },
        { name: "birefnet-general", inputSize: 1024, description: "Best quality, ~15s CPU" },
      ])
    );

    // Tauri v2 drag-drop events
    let unlisteners: UnlistenFn[] = [];

    // tauri://file-drop — payload can be string[] OR { paths: string[] } depending on API version
    listen<unknown>("tauri://file-drop", (ev) => {
      const payload = ev.payload;
      let paths: string[] = [];
      if (Array.isArray(payload)) {
        paths = payload;
      } else if (payload && typeof payload === "object" && "paths" in (payload as object)) {
        paths = ((payload as { paths: unknown }).paths as string[]) || [];
      }
      if (paths.length > 0) {
        addPathsRef.current(paths);
      }
      setDragOver(false);
    }).then((fn) => unlisteners.push(fn));

    listen("tauri://file-drop-hover", () => {
      setDragOver(true);
    }).then((fn) => unlisteners.push(fn));

    listen("tauri://file-drop-cancelled", () => {
      setDragOver(false);
    }).then((fn) => unlisteners.push(fn));

    listen<BatchProgress>("batch-progress", (ev) => {
      const p = ev.payload;
      setProgress({ current: p.current, total: p.total, phase: p.phase });
      setFiles((prev) => prev.map((f) => {
        if (f.path !== p.input) return f;
        if (p.phase.phase === "starting" || p.phase.phase === "inferring" || p.phase.phase === "saving")
          return { ...f, status: "processing" };
        if (p.phase.phase === "done" && p.phase.success) return { ...f, status: "done" };
        if (p.phase.phase === "done" && !p.phase.success)
          return { ...f, status: "error", error: p.phase.error };
        return f;
      }));
      if (p.phase.phase === "done" && p.current === p.total) {
        setIsProcessing(false);
        // 处理完成后保留进度条，停在 100% / Done
      }
    }).then((fn) => unlisteners.push(fn));

    // Stop button callback — fires when Rust breaks the loop
    listen("batch-cancelled", () => {
      setIsProcessing(false);
      cancelledRef.current = true;
      addToast("⏹ Batch stopped by user", "info");
    }).then((fn) => unlisteners.push(fn));

    return () => { unlisteners.forEach((fn) => fn()); };
  }, []);

  // Track files that had "running" status to detect when batch finishes
  const [prevReady, setPrevReady] = useState<FileItem[]>([]);
  useEffect(() => {
    const hasProcessing = files.some((f) => f.status === "processing");
    if (!hasProcessing && files.length > 0 && progress === null) {
      // Batch just finished (not cancelled — cancelledRef is separate)
      if (!cancelledRef.current && prevReady.length > 0) {
        const done = files.filter((f) => f.status === "done").length;
        const failed = files.filter((f) => f.status === "error").length;
        if (done + failed > 0) {
          addToast(`✓ Batch done: ${done} ok${failed ? `, ${failed} failed` : ""}`, failed > 0 ? "error" : "success");
          // 批处理完成后自动打开输出目录
          if (outputDir) {
            try { invoke("open_file", { path: outputDir }); } catch {}
          }
        }
      }
      cancelledRef.current = false;
    }
    setPrevReady(files);
  }, [files, progress, addToast, outputDir]);

  const addFiles = async () => {
    const selected = await open({
      multiple: true,
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp"] }],
    });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    await addPaths(paths as string[]);
  };

  const addPaths = useCallback(async (paths: string[]) => {
    if (!paths || paths.length === 0) return;
    // Expand any folders → flat image file list
    let expanded: string[] = paths;
    try {
      expanded = await invoke<string[]>("walk_paths", { paths });
    } catch { /* fallback: use as-is */ }
    if (expanded.length === 0) {
      addToast("No image files found (supported: png, jpg, jpeg, webp)", "error");
      return;
    }
    const newItems: FileItem[] = expanded.map((p) => ({
      id: crypto.randomUUID(),
      path: p,
      name: p.split(/[\\/]/).pop() || p,
      previewUrl: fileUrl(p),
      status: "pending" as const,
    }));
    setFiles((prev) => [...prev, ...newItems]);
    setOutputDir((prev) => {
      if (!prev && expanded.length > 0) return expanded[0].replace(/[/\\][^/\\]+$/, "");
      return prev;
    });
    addToast(`+ ${expanded.length} image${expanded.length > 1 ? "s" : ""} added`, "info");
  }, [addToast]);
  addPathsRef.current = addPaths; // keep ref in sync

  const removeFile = (id: string) => setFiles((prev) => prev.filter((f) => f.id !== id));
  const clearAll = () => {
    setFiles([]);
    setSelectedFileId(null);
    setPreviewNonce((n) => n + 1);
    setProgress(null);
  };

  const pickOutputDir = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (selected) setOutputDir(selected as string);
  };

  const addFolder = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (selected) await addPaths([selected as string]);
  };

  const pickWatermark = async () => {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Logo / Watermark", extensions: ["png", "webp", "jpg"] }],
    });
    if (selected) {
      const path = Array.isArray(selected) ? selected[0] : selected;
      setWmPath(path as string);
    }
  };

  // Shared opts builder — used by both processBatch (Pro) and processSingle (Community)
  const buildProcessOpts = () => {
    const preset = presets.find((p) => p.name === selectedPreset);
    const h = bgColor.replace("#", "");
    return {
      white_bg: whiteBg,
      shadow,
      shadow_style: shadowStyle,
      bg_r: parseInt(h.substring(0, 2), 16),
      bg_g: parseInt(h.substring(2, 4), 16),
      bg_b: parseInt(h.substring(4, 6), 16),
      center,
      target_w: preset?.w ?? 0,
      target_h: preset?.h ?? 0,
      watermark_path: wmPath || undefined,
      watermark_opacity: wmOpacity,
      watermark_position: wmPath ? wmPosition : undefined,
      model_name: selectedModel,
      format: outFormat,
      quality,
    };
  };

  // Community mode: process ONE image at a time (selectedFileId or first pending)
  const processSingle = async () => {
    if (files.length === 0 || !outputDir || isProcessing) return;
    const target = selectedFileId
      ? files.find((f) => f.id === selectedFileId && (f.status === "pending" || f.status === "error"))
      : files.find((f) => f.status === "pending" || f.status === "error");
    if (!target) { addToast("No pending images", "info"); return; }

    setIsProcessing(true);
    setFiles((prev) => prev.map((f) => f.id === target.id ? { ...f, status: "processing" } : f));

    const stem = target.name.replace(/\.[^.]+$/, "");
    const ext = outFormat === "jpg" ? "jpg" : outFormat;
    const outputPath = outputDir.endsWith("\\") || outputDir.endsWith("/")
      ? `${outputDir}${stem}.${ext}` : `${outputDir}/${stem}.${ext}`;

    try {
      const result = await invoke<{ success: boolean; error?: string; output: string }>(
        "process_image", { input: target.path, output: outputPath, opts: buildProcessOpts() }
      );
      setFiles((prev) => prev.map((f) => f.id === target.id ? {
        ...f,
        status: result.success ? "done" : "error",
        outputPath: result.output,
        outputUrl: result.success ? fileUrl(result.output) : undefined,
        error: result.error,
      } : f));
      setPreviewNonce((n) => n + 1);
      if (result.success) {
        addToast("✓ Done", "success");
        invoke("open_file", { path: outputDir }).catch(() => {});
      } else {
        addToast(`Failed: ${result.error}`, "error");
      }
    } catch (err) {
      addToast(`Error: ${String(err)}`, "error");
      setFiles((prev) => prev.map((f) => f.id === target.id ? { ...f, status: "error", error: String(err) } : f));
    } finally {
      setIsProcessing(false);
    }
  };

  const processBatch = async () => {
    if (files.length === 0 || !outputDir || isProcessing) return;
    cancelledRef.current = false;
    const preset = presets.find((p) => p.name === selectedPreset);
    // hex #rrggbb → [r,g,b]
    const h = bgColor.replace("#", "");
    const bgRGB = [parseInt(h.substring(0, 2), 16), parseInt(h.substring(2, 4), 16), parseInt(h.substring(4, 6), 16)];
    const opts = {
      white_bg: whiteBg,
      shadow,
      shadow_style: shadowStyle,
      bg_r: bgRGB[0], bg_g: bgRGB[1], bg_b: bgRGB[2],
      center,
      target_w: preset?.w ?? 0,
      target_h: preset?.h ?? 0,
      watermark_path: wmPath || undefined,
      watermark_opacity: wmOpacity,
      watermark_position: wmPath ? wmPosition : undefined,
      model_name: selectedModel,
      format: outFormat,
      quality,
    };

    const pending = files.filter((f) => f.status === "pending" || f.status === "error");
    setIsProcessing(true);
    setProgress({ current: 0, total: pending.length });
    setFiles((prev) => prev.map((f) =>
      pending.find((p) => p.id === f.id) ? { ...f, status: "processing" } : f
    ));

    try {
      const results = await invoke<
        { input: string; output: string; success: boolean; error?: string }[]
      >("batch_process", { inputs: pending.map((f) => f.path), outputDir, opts });
      setFiles((prev) => prev.map((f) => {
        const r = results.find((r) => r.input === f.path);
        if (r) return {
          ...f,
          status: r.success ? "done" : "error",
          outputPath: r.output,
          outputUrl: r.success ? fileUrl(r.output) : undefined,
          error: r.error,
        };
        return f; // cancelled / not reached → keep pending
      }));
    } catch (err) {
      const msg = String(err);
      setFiles((prev) => prev.map((f) =>
        f.status === "processing" ? { ...f, status: "error", error: msg } : f
      ));
    } finally {
      setPreviewNonce((n) => n + 1);
      setIsProcessing(false);
      setProgress(null); // 清零后 batch-done toast 逻辑才会触发
    }
  };

  const openOutput = async (path?: string) => {
    if (!path) return;
    try { await invoke("open_file", { path }); } catch { addToast("⚠ Cannot open file", "error"); }
  };
  const revealOutput = async (path?: string) => {
    if (!path) return;
    try { await invoke("reveal_in_folder", { path }); } catch { addToast("⚠ Cannot open folder", "error"); }
  };

  const pendingCount = files.filter((f) => f.status === "pending" || f.status === "error").length;
  const doneCount = files.filter((f) => f.status === "done").length;
  const selectedFile = files.find((f) => f.id === selectedFileId) ?? files[0];
  const showGallery = files.length > 0 && (doneCount > 0 || isProcessing);
  const preset = presets.find((p) => p.name === selectedPreset);
  const sizeLabel = preset && preset.w > 0 ? `${preset.w}×${preset.h}` : "Original";

  // 处理中的那一张还没完成，不能算进进度；否则一开始就显示 100%，让人误以为已经处理完
  const pct = progress && progress.total > 0
    ? Math.round(
        ((progress.phase?.phase === "done" ? progress.current : progress.current - 1) /
          progress.total) * 100
      )
    : 0;

  return (
    <div className="h-full flex flex-col bg-[#0a0a10]">
      {/* Custom black title bar — 整条 header 手动触发拖拽，按钮用 data-no-drag 排除 */}
      <header
        onMouseDown={(e) => {
          if (e.button !== 0) return;
          if ((e.target as HTMLElement).closest("[data-no-drag]")) return;
          getCurrentWindow().startDragging().catch(() => {});
        }}
        onDoubleClick={async (e) => {
          if ((e.target as HTMLElement).closest("[data-no-drag]")) return;
          try {
            const w = getCurrentWindow();
            if (await w.isMaximized()) await w.unmaximize(); else await w.maximize();
          } catch {}
        }}
        className="h-9 bg-black flex items-center select-none border-b border-white/5 cursor-default">
        <div className="flex-1 flex items-center px-4 gap-3 h-full">
          <div className="w-5 h-5 rounded-md bg-gradient-to-br from-brand-500 to-brand-700 flex items-center justify-center text-[10px]">✨</div>
          <h1 className="text-xs font-semibold tracking-tight text-white/90">MagiDiva</h1>
          <span className="text-[10px] text-white/30">— AI Product Image Magic</span>
          {isPro && (
            <span className="text-[10px] font-semibold px-1.5 py-0.5 rounded bg-gradient-to-r from-amber-400 to-orange-400 text-black">PRO</span>
          )}
          <span className="text-[10px] text-white/20 ml-auto">{appVersion}</span>
        </div>
        <div className="flex h-full">
          <button data-no-drag onClick={() => getCurrentWindow().minimize()}
            className="w-11 h-full hover:bg-white/5 text-white/60 hover:text-white transition flex items-center justify-center text-sm">—</button>
          <button data-no-drag onClick={async () => { const w = getCurrentWindow(); if (await w.isMaximized()) await w.unmaximize(); else await w.maximize(); }}
            className="w-11 h-full hover:bg-white/5 text-white/60 hover:text-white transition flex items-center justify-center text-[10px]">▢</button>
          <button data-no-drag onClick={() => getCurrentWindow().close()}
            className="w-11 h-full hover:bg-red-600 text-white/60 hover:text-white transition flex items-center justify-center text-sm">✕</button>
        </div>
      </header>

      <main className="flex-1 flex overflow-hidden">
        {/* Left controls */}
        <aside className="w-[360px] border-r border-white/5 bg-[#0e0e16] flex flex-col overflow-y-auto">
          <div className="p-4 border-b border-white/5">
            <label className="text-xs text-white/50 mb-1 block">Output Folder</label>
            <div className="flex gap-2">
              <input value={outputDir} readOnly placeholder="Pick output folder..."
                className="flex-1 bg-white/5 border border-white/10 rounded px-3 py-2 text-sm text-white/80 placeholder-white/30" />
              <button onClick={pickOutputDir} className="px-3 py-2 text-sm rounded bg-brand-600 hover:bg-brand-500 transition">Browse</button>
            </div>
          </div>

          <div className="p-4 border-b border-white/5 space-y-3">
            <label className="text-xs text-white/50 uppercase tracking-wider">Processing</label>
            <label className="flex items-center gap-2">
              <input type="checkbox" checked={center} onChange={(e) => setCenter(e.target.checked)} className="w-4 h-4 accent-brand-500" />
              <span className="text-sm text-white/80">Auto-center product</span>
            </label>
            <label className="flex items-center gap-2">
              <input type="checkbox" checked={whiteBg}
                onChange={(e) => { const v = e.target.checked; setWhiteBg(v); if (!v) setShadow(false); }}
                className="w-4 h-4 accent-brand-500" />
              <span className="text-sm text-white/80">Background color</span>
            </label>
            {whiteBg && (
              <div className="flex items-center gap-2">
                <input type="color" value={bgColor} onChange={(e) => setBgColor(e.target.value)}
                  className="w-8 h-8 rounded cursor-pointer border-0 bg-transparent" />
                <div className="flex gap-1">
                  {[
                    ["#ffffff", "White"], ["#f5f5f5", "Light"], ["#d0d0d0", "Gray"],
                    ["#1a1a24", "Dark"], ["#ff6b6b", "Red"], ["#4dabf7", "Blue"],
                  ].map(([c, label]) => (
                    <button key={c} title={label} onClick={() => setBgColor(c)}
                      className={`w-6 h-6 rounded border transition-transform hover:scale-110 ${bgColor === c ? "ring-2 ring-brand-500" : "border-white/20"}`}
                      style={{ backgroundColor: c }} />
                  ))}
                </div>
              </div>
            )}
            <label className={`flex items-center gap-2 ${!whiteBg ? "opacity-40" : ""}`}>
              <input type="checkbox" checked={shadow} disabled={!whiteBg}
                onChange={(e) => setShadow(e.target.checked)} className="w-4 h-4 accent-brand-500" />
              <span className="text-sm text-white/80">Drop shadow</span>
            </label>
            {shadow && (
              <div>
                <label className="text-xs text-white/50 mb-1 block">Shadow style</label>
                <select value={shadowStyle} onChange={(e) => setShadowStyle(e.target.value as any)}
                  className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-sm text-white/80">
                  <option value="soft" className="bg-[#1a1a24]">Soft — 柔和透明</option>
                  <option value="medium" className="bg-[#1a1a24]">Medium — 平衡 (推荐)</option>
                  <option value="strong" className="bg-[#1a1a24]">Strong — 厚重投影</option>
                  <option value="offset" className="bg-[#1a1a24]">Offset — 硬偏移</option>
                </select>
              </div>
            )}
            <div>
              <label className="text-xs text-white/50 mb-1 block">AI Model</label>
              <select value={selectedModel} onChange={(e) => setSelectedModel(e.target.value)}
                className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-sm text-white/80">
                {models.map((m) => (
                  <option key={m.name} value={m.name} className="bg-[#1a1a24]">
                    {m.name} — {m.description}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="text-xs text-white/50 mb-1 block">Canvas Size</label>
              <select value={selectedPreset} onChange={(e) => setSelectedPreset(e.target.value)}
                className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-sm text-white/80">
                {presets.map((p) => (
                  <option key={p.name} value={p.name} className="bg-[#1a1a24]">{p.name}</option>
                ))}
              </select>
            </div>
            <div>
              <label className="text-xs text-white/50 mb-1 block">Output Format</label>
              <select value={outFormat} onChange={(e) => setOutFormat(e.target.value as "png" | "jpg" | "webp")}
                className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-sm text-white/80">
                <option value="png" className="bg-[#1a1a24]">PNG — lossless, transparent</option>
                <option value="jpg" className="bg-[#1a1a24]">JPG — smallest, no transparency</option>
                <option value="webp" className="bg-[#1a1a24]">WebP — lossless, small</option>
              </select>
            </div>
            {outFormat === "jpg" && (
              <div>
                <div className="flex items-center justify-between text-xs text-white/50 mb-1">
                  <span>JPG Quality</span>
                  <span className="text-white/70">{quality}</span>
                </div>
                <input type="range" min={60} max={100} step={1} value={quality}
                  onChange={(e) => setQuality(parseInt(e.target.value, 10))}
                  className="w-full accent-brand-500" />
              </div>
            )}
          </div>

          <div className="p-4 border-b border-white/5 space-y-3">
            <label className="text-xs text-white/50 uppercase tracking-wider">Watermark (optional)</label>
            <div className="flex gap-2">
              <input value={wmPath} readOnly placeholder="No logo selected"
                className="flex-1 bg-white/5 border border-white/10 rounded px-3 py-2 text-xs text-white/70 placeholder-white/30 min-w-0 truncate" />
              <button onClick={pickWatermark} className="px-3 py-2 text-xs rounded bg-white/10 hover:bg-white/20 transition">Pick</button>
              {wmPath && <button onClick={() => setWmPath("")} className="px-2 py-2 text-xs rounded hover:text-red-400 text-white/40">✕</button>}
            </div>
            {wmPath && (
              <>
                <div>
                  <div className="flex items-center justify-between text-xs text-white/50 mb-1">
                    <span>Opacity</span>
                    <span className="text-white/70">{Math.round(wmOpacity * 100)}%</span>
                  </div>
                  <input type="range" min={0.1} max={1.0} step={0.05} value={wmOpacity}
                    onChange={(e) => setWmOpacity(parseFloat(e.target.value))}
                    className="w-full accent-brand-500" />
                </div>
                <div>
                  <label className="text-xs text-white/50 mb-1 block">Position</label>
                  <select value={wmPosition} onChange={(e) => setWmPosition(e.target.value)}
                    className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-sm text-white/80">
                    {wmPositions.map((p) => (
                      <option key={p.value} value={p.value} className="bg-[#1a1a24]">{p.label}</option>
                    ))}
                  </select>
                </div>
              </>
            )}
          </div>

          <div onClick={addFiles}
            className={`m-4 rounded-lg border-2 border-dashed transition cursor-pointer text-center py-6 ${
              dragOver ? "border-brand-500 bg-brand-500/10" : "border-white/15 hover:border-white/30"
            }`}>
            <div className="text-2xl mb-1">📥</div>
            <p className="text-sm text-white/70">Drop images or folders here</p>
            <p className="text-xs text-white/40 mt-1">or click to browse files</p>
          </div>

          <div className="px-4 pb-2">
            {isPro ? (
              <button onClick={addFolder}
                className="w-full py-2 text-xs rounded border border-white/15 hover:border-white/30 hover:bg-white/5 transition text-white/70">
                📂 Add Folder (recursive)
              </button>
            ) : (
              <button onClick={() => window.open("https://magidiva.com/pro", "_blank")}
                className="w-full py-2 text-xs rounded border border-amber-400/30 bg-amber-400/5 hover:bg-amber-400/10 transition text-amber-300">
                🔒 Batch & Folder · Pro only →
              </button>
            )}
          </div>

          {/* File list with real thumbnails */}
          <div className="flex-1 overflow-y-auto px-4 pb-2">
            <div className="text-xs text-white/50 mb-2">{files.length} files · {doneCount} done</div>
            {files.length === 0 && <div className="text-center text-white/30 text-xs py-8">No files yet</div>}
            <div className="space-y-2">
              {files.map((f) => (
                <div key={f.id} className="flex items-center gap-2 p-2 rounded bg-white/5 group hover:bg-white/10 transition">
                  {/* Thumbnail */}
                  <div className="w-10 h-10 rounded bg-[#1a1a24] flex-shrink-0 overflow-hidden border border-white/10 flex items-center justify-center">
                    <img src={f.previewUrl} alt="" className="w-full h-full object-cover"
                      onError={(e) => {
                        const tgt = e.target as HTMLImageElement;
                        tgt.style.display = "none";
                        const parent = tgt.parentElement;
                        if (parent && !parent.querySelector(".fallback")) {
                          const fb = document.createElement("div");
                          fb.className = "fallback text-brand-400 text-[10px] font-bold";
                          fb.textContent = f.name.substring(0, 2).toUpperCase();
                          parent.appendChild(fb);
                        }
                      }} />
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="text-xs truncate text-white/80">{f.name}</div>
                    <div className="text-[10px] mt-0.5">
                      {f.status === "pending" && <span className="text-white/40">Ready</span>}
                      {f.status === "processing" && <span className="text-yellow-400 animate-pulse">Running…</span>}
                      {f.status === "done" && (
                        <span className="text-green-400 cursor-pointer hover:underline"
                          onClick={() => openOutput(f.outputPath)} title="Click to open output">
                          ✓ Click to open
                        </span>
                      )}
                      {f.status === "error" && <span className="text-red-400">✗ {f.error || "Failed"}</span>}
                    </div>
                  </div>
                  {/* Output actions */}
                  {f.status === "done" && f.outputPath && (
                    <>
                      <button onClick={(e) => { e.stopPropagation(); revealOutput(f.outputPath); }}
                        className="text-white/30 hover:text-white text-xs opacity-0 group-hover:opacity-100 transition"
                        title="Reveal in folder">📁</button>
                      <button onClick={(e) => { e.stopPropagation(); openOutput(f.outputPath); }}
                        className="text-white/30 hover:text-white text-xs opacity-0 group-hover:opacity-100 transition"
                        title="Open output">↗</button>
                    </>
                  )}
                  <button onClick={() => removeFile(f.id)}
                    className="opacity-0 group-hover:opacity-100 text-white/40 hover:text-red-400 text-sm">✕</button>
                </div>
              ))}
            </div>
          </div>

          <div className="p-4 border-t border-white/5 flex gap-2">
            {isPro ? (
              isProcessing ? (
                <button onClick={() => invoke("cancel_batch")}
                  className="flex-1 py-3 rounded-lg font-semibold bg-red-600/80 hover:bg-red-500 transition">
                  ⏹ Stop batch
                </button>
              ) : (
                <button onClick={processBatch} disabled={pendingCount === 0 || !outputDir}
                  className="flex-1 py-3 rounded-lg font-semibold bg-gradient-to-r from-brand-600 to-brand-500 hover:from-brand-500 hover:to-brand-400 disabled:opacity-40 disabled:cursor-not-allowed transition">
                  ✨ Process {pendingCount} · {sizeLabel}
                </button>
              )
            ) : (
              <button onClick={processSingle} disabled={pendingCount === 0 || !outputDir || isProcessing}
                className="flex-1 py-3 rounded-lg font-semibold bg-gradient-to-r from-brand-600 to-brand-500 hover:from-brand-500 hover:to-brand-400 disabled:opacity-40 disabled:cursor-not-allowed transition">
                {isProcessing ? "Processing…" : `✨ Process 1 · ${sizeLabel}`}
              </button>
            )}
          </div>
        </aside>

        {/* Right: single preview before run, gallery after */}
        <section className="flex-1 flex flex-col checkerboard relative min-h-0">
          {files.length === 0 && (
            <div className="flex-1 flex items-center justify-center text-white/40">
              <div className="text-center">
                <div className="text-6xl mb-4">🖼️</div>
                <p className="text-lg">Drop some product images</p>
                <p className="text-sm mt-1">Remove background, auto-center, white background, watermark</p>
              </div>
            </div>
          )}

          {showGallery && (
            <>
              {/* Top: big preview of selected file */}
              <div className="flex-1 flex items-center justify-center p-6 min-h-0">
                {selectedFile && (
                  <div className="w-full h-full flex flex-col max-w-4xl">
                    <div className="flex items-center justify-between mb-2 gap-3">
                      <div className="text-xs text-white/50 uppercase tracking-wider truncate flex-1">
                        {selectedFile.name}
                        {selectedFile.status === "done" && selectedFile.outputPath && (
                          <button
                            onClick={() => { try { invoke("reveal_in_folder", { path: selectedFile.outputPath! }); } catch {} }}
                            className="ml-2 text-[10px] text-white/40 hover:text-brand-400 underline">
                            Open output ↗
                          </button>
                        )}
                      </div>
                      <div className="text-[10px] text-white/30 flex-shrink-0">
                        {sizeLabel}{whiteBg ? " · White" : ""}{shadow ? " · Shadow" : ""}{center ? " · Centered" : ""}{wmPath ? " · Wm" : ""} · {outFormat.toUpperCase()}
                      </div>
                      {!isProcessing && (
                        <button onClick={clearAll}
                          className="flex-shrink-0 px-2 py-1 text-[10px] rounded border border-white/10 hover:border-red-400/50 hover:text-red-400 text-white/40 transition">
                          ✕ Clear all
                        </button>
                      )}
                    </div>
                    <div className="flex-1 rounded-lg overflow-hidden flex items-center justify-center border border-white/10 bg-black/30 min-h-0 relative">
                      {selectedFile.status === "processing" && (
                        <div className="absolute inset-0 flex items-center justify-center bg-black/60 z-10">
                          <div className="flex flex-col items-center gap-2 text-brand-300">
                            <div className="w-8 h-8 border-2 border-brand-400 border-t-transparent rounded-full animate-spin" />
                            <span className="text-xs">Processing...</span>
                          </div>
                        </div>
                      )}
                      {selectedFile.status === "done" && selectedFile.outputPath ? (
                        <img src={`${fileUrl(selectedFile.outputPath)}?v=${previewNonce}`} alt="output"
                          className="max-w-full max-h-full object-contain"
                          onError={(e) => { (e.target as HTMLImageElement).style.opacity = "0.4"; }} />
                      ) : selectedFile.status === "error" ? (
                        <div className="text-center text-red-400 px-6">
                          <div className="text-3xl mb-2">⚠️</div>
                          <p className="text-sm">{selectedFile.error ?? "Processing failed"}</p>
                        </div>
                      ) : (
                        <div className="text-white/30 text-sm">
                          {selectedFile.previewUrl && (
                            <img src={selectedFile.previewUrl} alt=""
                              className="max-w-full max-h-full object-contain opacity-40" />
                          )}
                          <div className="absolute bottom-4 left-0 right-0 text-center text-xs">
                            Waiting — run batch to see output
                          </div>
                        </div>
                      )}
                    </div>
                  </div>
                )}
              </div>

              {/* Bottom: thumbnail strip */}
              <div className="border-t border-white/5 bg-black/20 px-4 py-3">
                <div className="flex gap-2 overflow-x-auto pb-1" style={{ scrollbarWidth: "thin" }}>
                  {files.map((f) => {
                    const isSel = f.id === selectedFile?.id;
                    const thumbSrc = f.status === "done" && f.outputPath
                      ? `${fileUrl(f.outputPath)}?v=${previewNonce}`
                      : f.previewUrl;
                    const badge = (() => {
                      switch (f.status) {
                        case "done": return { label: "\u2713", cls: "bg-green-500/80 text-white" };
                        case "processing": return { label: "\u2026", cls: "bg-brand-500/80 text-white animate-pulse" };
                        case "error": return { label: "!", cls: "bg-red-500/80 text-white" };
                        default: return { label: "", cls: "" };
                      }
                    })();
                    return (
                      <button key={f.id}
                        onClick={() => setSelectedFileId(f.id)}
                        className={"flex-shrink-0 relative rounded-md overflow-hidden transition-all " +
                          (isSel
                            ? "ring-2 ring-brand-400 ring-offset-2 ring-offset-[#0a0a10] scale-105"
                            : "ring-1 ring-white/10 hover:ring-white/30")}
                        title={f.name}>
                        <div className="w-20 h-20 bg-black/40 flex items-center justify-center">
                          {thumbSrc ? (
                            <img src={thumbSrc} alt="" className="w-full h-full object-cover"
                              onError={(e) => { (e.target as HTMLImageElement).style.visibility = "hidden"; }} />
                          ) : (
                            <span className="text-white/20 text-xs">?</span>
                          )}
                        </div>
                        {badge.label && (
                          <span className={"absolute top-1 right-1 w-5 h-5 rounded-full text-[10px] font-bold flex items-center justify-center " + badge.cls}>
                            {badge.label}
                          </span>
                        )}
                      </button>
                    );
                  })}
                </div>
              </div>
            </>
          )}

          {files.length > 0 && !showGallery && (
            <div className="flex-1 flex items-center justify-center p-6">
              <div className="max-w-4xl w-full flex flex-col items-center">
                <div className="text-xs text-white/50 mb-2 uppercase tracking-wider">Input Preview</div>
                <div className="rounded-lg overflow-hidden border border-white/10 bg-black/30 w-full max-h-[70vh] flex items-center justify-center">
                  {files[0].previewUrl && (
                    <img src={files[0].previewUrl} alt="input"
                      className="max-w-full max-h-[70vh] object-contain" />
                  )}
                </div>
                <div className="text-[10px] text-white/40 mt-3 text-center">
                  {files.length} file{files.length > 1 ? "s" : ""} queued · {sizeLabel}{whiteBg ? " · White" : ""}{shadow ? " · Shadow" : ""}{center ? " · Centered" : ""}{wmPath ? " · Wm" : ""} · {outFormat.toUpperCase()}
                </div>
                <div className="mt-4 text-xs text-white/30">
                  Click <span className="text-brand-400 font-semibold">✨ Process</span> to run batch
                </div>
              </div>
            </div>
          )}
        </section>
      </main>

      {/* Progress — 固定在窗口最下方 */}
      {progress && (
        <div className="px-6 py-3 bg-brand-500/10 border-t border-brand-500/20 animate-fade-in">
          <div className="flex items-center justify-between text-xs text-brand-300 mb-2">
            <span className="flex items-center gap-2">
              <span className="w-2 h-2 rounded-full bg-brand-400 animate-pulse" />
              {progress.phase ? phaseLabel(progress.phase) : "Processing..."}
            </span>
            <span className="text-white/50">
              {progress.current} / {progress.total} · {pct}%
            </span>
          </div>
          <div className="h-2 rounded-full bg-white/10 overflow-hidden">
            <div className="h-full bg-gradient-to-r from-brand-600 to-brand-400 transition-all duration-500 ease-out"
              style={{ width: `${pct}%` }} />
          </div>
        </div>
      )}

      {/* Toast notifications */}
      <div className="fixed bottom-6 right-6 flex flex-col gap-2 z-50 pointer-events-none">
        {toasts.map((t) => (
          <div key={t.id}
            className={`pointer-events-auto px-4 py-2.5 rounded-lg text-sm shadow-xl border animate-fade-in ${
              t.type === "success" ? "bg-green-900/80 border-green-500/40 text-green-100" :
              t.type === "error" ? "bg-red-900/80 border-red-500/40 text-red-100" :
              "bg-[#1a1a24] border-white/10 text-white/80"
            }`}>
            {t.msg}
          </div>
        ))}
      </div>
    </div>
  );
}

export default App;
