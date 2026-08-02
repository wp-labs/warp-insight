import { useEffect, useRef, useState } from "react";

interface CopyButtonProps {
  text?: string;
  label: string;
  copiedLabel?: string;
  className?: string;
  disabled?: boolean;
}

export function CopyButton({
  text,
  label,
  copiedLabel = "已复制",
  className,
  disabled,
}: CopyButtonProps) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  function handleCopy() {
    if (!text) {
      return;
    }
    navigator.clipboard
      ?.writeText(text)
      .then(() => {
        setCopied(true);
        window.clearTimeout(timer.current);
        timer.current = window.setTimeout(() => setCopied(false), 1500);
      })
      .catch(() => {});
  }

  return (
    <button
      type="button"
      className={className}
      disabled={disabled ?? !text}
      onClick={handleCopy}
    >
      {copied ? copiedLabel : label}
    </button>
  );
}
