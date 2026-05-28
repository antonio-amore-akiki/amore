"use client";

import { useState } from "react";
import type { InstallCommand } from "@/lib/products";

type OS = "windows" | "macos" | "linux";

interface OSToggleProps {
  commands: {
    windows: InstallCommand[];
    macos: InstallCommand[];
    linux: InstallCommand[];
  };
  defaultOS?: OS;
}

export function OSToggle({ commands, defaultOS = "windows" }: OSToggleProps) {
  const [active, setActive] = useState<OS>(defaultOS);

  const tabs: { key: OS; label: string }[] = [
    { key: "windows", label: "Windows" },
    { key: "macos", label: "macOS" },
    { key: "linux", label: "Linux" },
  ];

  return (
    <div>
      <div className="flex gap-1 border-b border-gray-200 mb-4">
        {tabs.map((tab) => (
          <button
            key={tab.key}
            onClick={() => setActive(tab.key)}
            className={[
              "px-4 py-2 text-sm font-medium rounded-t",
              active === tab.key
                ? "bg-white border border-b-white border-gray-200 -mb-px text-blue-600"
                : "text-gray-600 hover:text-gray-900",
            ].join(" ")}
          >
            {tab.label}
          </button>
        ))}
      </div>
      <div className="space-y-3">
        {commands[active].map((cmd) => (
          <InstallRow key={cmd.channel} cmd={cmd} />
        ))}
      </div>
    </div>
  );
}

interface InstallRowProps {
  cmd: InstallCommand;
}

function InstallRow({ cmd }: InstallRowProps) {
  const [copied, setCopied] = useState(false);

  async function handleCopy() {
    if (cmd.type !== "shell") return;
    try {
      await navigator.clipboard.writeText(cmd.command);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // clipboard unavailable (non-HTTPS dev) — silent, no fallback needed for MVP
    }
  }

  return (
    <div className="flex items-start gap-3 p-3 bg-gray-50 rounded-lg border border-gray-100">
      <div className="flex-1 min-w-0">
        <div className="text-xs font-semibold text-gray-500 uppercase tracking-wide mb-1">
          {cmd.label}
        </div>
        <code className="text-sm text-gray-800 break-all">{cmd.command}</code>
      </div>
      {cmd.type === "shell" && (
        <button
          onClick={handleCopy}
          aria-label={copied ? "Copied" : "Copy command"}
          className="flex-shrink-0 px-3 py-1.5 text-xs font-medium rounded bg-gray-200 hover:bg-gray-300 text-gray-700 transition-colors"
        >
          {copied ? "Copied!" : "Copy"}
        </button>
      )}
    </div>
  );
}
