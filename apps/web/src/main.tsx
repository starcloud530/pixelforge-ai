import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { initKernel } from "./kernel/client";
import "./styles.css";

const root = document.getElementById("root");
if (!root) {
  throw new Error("missing #root");
}

root.textContent = "正在加载内核…";

void initKernel()
  .then(() => {
    createRoot(root).render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
  })
  .catch((error: unknown) => {
    console.error(error);
    root.textContent = error instanceof Error ? `内核加载失败：${error.message}` : "内核加载失败";
  });
