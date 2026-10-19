import { useEffect, useRef, useState } from "react";
import type { PixelKernel } from "../kernel/types";
import { OpenAiCompatibleProvider } from "./provider";
import type { AiProxyStatus } from "./proxy";
import { runAgentTurn, type AgentStep, type AgentTurnResult } from "./turn";

/** Same-origin root. The dev server attaches the API key on the way upstream. */
const LOCAL_AI_ROOT = "/api/ai";

interface AiPanelProps {
  kernel: PixelKernel;
  onBusy: (busy: boolean) => void;
  /** Stop playback before the turn so the clock does not move the active frame. */
  onBeforeRun: () => void;
  /** The kernel changed. The canvas should read a new snapshot. */
  onApplied: () => void;
}

/**
 * One prompt, one model reply, then Tool API calls on the open document.
 * The panel does not touch pixel buffers. Refresh comes from `onApplied`.
 */
export function AiPanel({ kernel, onBusy, onBeforeRun, onApplied }: AiPanelProps) {
  const [status, setStatus] = useState<AiProxyStatus | null>(null);
  const [statusNote, setStatusNote] = useState<string | null>(null);
  const [prompt, setPrompt] = useState("");
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<AgentTurnResult | null>(null);
  const runId = useRef(0);
  const kernelRef = useRef(kernel);
  kernelRef.current = kernel;

  useEffect(() => {
    let cancelled = false;
    void fetch(`${LOCAL_AI_ROOT}/status`)
      .then(async (response) => {
        if (!response.ok) {
          throw new Error("status");
        }
        return (await response.json()) as unknown;
      })
      .then((value) => {
        if (cancelled) {
          return;
        }
        if (!isStatus(value)) {
          setStatusNote("本地模型设置读不出来。用 pnpm dev 或 pnpm preview 打开这个页面。");
          return;
        }
        setStatus(value);
      })
      .catch(() => {
        if (!cancelled) {
          setStatusNote("本地模型设置读不出来。用 pnpm dev 或 pnpm preview 打开这个页面。");
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    setResult(null);
  }, [kernel]);

  useEffect(() => {
    return () => {
      runId.current += 1;
    };
  }, []);

  async function runTurn() {
    if (!status?.configured || running) {
      return;
    }
    const activeKernel = kernel;
    const id = runId.current + 1;
    runId.current = id;
    setRunning(true);
    onBusy(true);
    onBeforeRun();
    try {
      const turn = await runAgentTurn({
        prompt,
        kernel: activeKernel,
        model: new OpenAiCompatibleProvider({
          apiKey: "local-proxy",
          baseUrl: LOCAL_AI_ROOT,
        }),
        modelName: status.model,
      });
      if (runId.current !== id || kernelRef.current !== activeKernel) {
        return;
      }
      setResult(turn);
      if (turn.status === "completed" || turn.status === "partial") {
        onApplied();
      }
    } catch (error) {
      if (runId.current !== id) {
        return;
      }
      const message = error instanceof Error && error.message ? error.message : "这一轮没有跑完。";
      setResult({
        status: "provider_error",
        model: status.model,
        assistantText: null,
        message,
        steps: [],
        wrote: false,
      });
    } finally {
      if (runId.current === id) {
        setRunning(false);
        onBusy(false);
      }
    }
  }

  const modelSteps = result?.steps.filter((entry) => entry.source === "model") ?? [];

  return (
    <section className="ai-panel" aria-label="外部模型">
      <h2>外部模型</h2>
      <label className="ai-prompt">
        <span className="hint">一轮提示。修改只通过 Tool API 写入内核。</span>
        <textarea
          value={prompt}
          placeholder="例如：把 (0,0) 涂成索引 3，再加一帧空白帧"
          disabled={running}
          onChange={(event) => setPrompt(event.target.value)}
        />
      </label>
      <div className="tool-row">
        <button
          type="button"
          disabled={!status?.configured || running || prompt.trim() === ""}
          onClick={() => {
            void runTurn();
          }}
        >
          {running ? "正在运行…" : "运行一轮"}
        </button>
        {status?.configured ? <span className="hint">{status.model}</span> : null}
      </div>
      {statusNote ? (
        <p className="hint">{statusNote}</p>
      ) : !status ? (
        <p className="hint">正在读取本地模型设置…</p>
      ) : !status.configured ? (
        <p className="hint">
          未设置 PIXELFORGE_AI_API_KEY 或 OPENAI_API_KEY。写入 apps/web/.env.local 后重新启动开发服务器。密钥只留在这个服务上，不会进页面。
        </p>
      ) : (
        <p className="hint">
          接口 {status.baseUrl}。页面把这一轮交给本地服务，密钥不进浏览器包。失败的调用不会改文档；已经写上的内容可以用撤销退回。
        </p>
      )}
      {result ? <TurnReport result={result} steps={modelSteps} /> : null}
    </section>
  );
}

function TurnReport({ result, steps }: { result: AgentTurnResult; steps: AgentStep[] }) {
  const grouped = result.steps.some((entry) => entry.id === "driver-begin" && entry.response?.ok);
  return (
    <div className="ai-report" aria-live="polite">
      {result.message ? (
        <p className={result.status === "completed" ? "hint" : "alert"} role="status">
          {result.message}
          {result.status === "partial" && result.wrote ? " 已写上的部分可以用撤销退回。" : ""}
        </p>
      ) : null}
      {result.assistantText ? <p className="hint">{result.assistantText}</p> : null}
      {result.status === "completed" && result.wrote ? (
        <p className="hint">
          {grouped
            ? "已写入内核。这一轮收成一步撤销。"
            : "已写入内核。撤销退回最近一步。"}
        </p>
      ) : null}
      {steps.length > 0 ? (
        <ol className="ai-log">
          {steps.map((entry) => (
            <li key={entry.id} className={stepFailed(entry) ? "is-error" : undefined}>
              <code>{entry.tool}</code>
              {entry.params ? <code>{` ${JSON.stringify(entry.params)}`}</code> : null}
              <span>{` · ${summarizeStep(entry)}`}</span>
            </li>
          ))}
        </ol>
      ) : null}
    </div>
  );
}

function isStatus(value: unknown): value is AiProxyStatus {
  if (typeof value !== "object" || value === null) {
    return false;
  }
  const record = value as Record<string, unknown>;
  return (
    typeof record.configured === "boolean" &&
    typeof record.model === "string" &&
    (record.baseUrl === null || typeof record.baseUrl === "string")
  );
}

function stepFailed(entry: AgentStep): boolean {
  return entry.driverError !== undefined || entry.response?.ok === false;
}

function summarizeStep(entry: AgentStep): string {
  if (entry.driverError) {
    return entry.driverError.message;
  }
  const response = entry.response;
  if (!response) {
    return "没有执行";
  }
  if (!response.ok) {
    return `${response.error.code}：${response.error.message}`;
  }
  const body = response.result;
  if (typeof body.index === "number") {
    return `ok · index ${body.index}`;
  }
  if ("frames" in body || "png" in body) {
    return "ok · 已导出";
  }
  if (typeof body.json === "string") {
    return "ok · 工程文本";
  }
  if (typeof body.canUndo === "boolean") {
    return `ok · canUndo ${body.canUndo ? "true" : "false"}`;
  }
  return "ok";
}
