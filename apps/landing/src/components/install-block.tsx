"use client";

import { useState } from "react";
import { ClipboardIcon, CheckIcon } from "@/components/ui/icons";

export const INSTALL_COMMAND =
  "curl -fsSL https://kdb.digimata.dev/install | bash";

export function InstallBlock() {
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    await navigator.clipboard.writeText(INSTALL_COMMAND);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="flex flex-col gap-4">
      <h2 className="text-heading-14 text-ds-gray-1000">Installation</h2>
      <div className="flex items-center gap-2 overflow-hidden rounded-md border border-ds-gray-300 bg-ds-bg-300 px-4 py-3">
        <span className="min-w-0 flex-1 overflow-x-auto whitespace-nowrap font-mono text-[13px] text-ds-prose">
          {INSTALL_COMMAND}
        </span>
        <button
          onClick={copy}
          className="shrink-0 cursor-pointer rounded-md border border-ds-gray-300 bg-ds-bg-200 p-1.5 text-ds-gray-600 transition-colors hover:text-ds-gray-1000"
          aria-label={copied ? "Copied" : "Copy install command"}
        >
          {copied ? <CheckIcon /> : <ClipboardIcon />}
        </button>
      </div>
      <p className="text-copy-13 text-ds-gray-600">
        Prebuilt binaries for macOS and Linux. Or install from source with{" "}
        <code className="font-mono text-ds-gray-1000">
          cargo install --path .
        </code>
        .
      </p>
    </div>
  );
}
