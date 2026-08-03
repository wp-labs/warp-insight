import { useEffect, useRef, useState } from "react";

interface CopyButtonProps {
  text?: string;
  label: string;
  copiedLabel?: string;
  failedLabel?: string;
  className?: string;
  disabled?: boolean;
}

export async function copyText(text: string): Promise<void> {
  // The async Clipboard API is only available in secure contexts (HTTPS or
  // localhost). Over the LAN the admin web runs as plain http://<ip>:5173, so
  // fall back to execCommand("copy") via a temporary textarea.
  if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.top = "0";
  textarea.style.left = "0";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.focus();
  textarea.select();
  const ok = document.execCommand("copy");
  document.body.removeChild(textarea);
  if (!ok) {
    throw new Error("copy failed");
  }
}

export function CopyButton({
  text,
  label,
  copiedLabel = "已复制",
  failedLabel = "复制失败",
  className,
  disabled,
}: CopyButtonProps) {
  const [state, setState] = useState<"idle" | "copied" | "failed">("idle");
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  function resetSoon() {
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setState("idle"), 1500);
  }

  function handleCopy() {
    if (!text) {
      return;
    }
    copyText(text)
      .then(() => {
        setState("copied");
        resetSoon();
      })
      .catch(() => {
        setState("failed");
        resetSoon();
      });
  }

  const currentLabel =
    state === "copied" ? copiedLabel : state === "failed" ? failedLabel : label;

  return (
    <button
      type="button"
      className={className}
      disabled={disabled ?? !text}
      onClick={handleCopy}
    >
      {currentLabel}
    </button>
  );
}
