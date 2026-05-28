"use client";

import { useEffect, useState } from "react";
import { OSToggle } from "@/components/OSToggle";
import type { Product } from "@/lib/products";

type OS = "windows" | "macos" | "linux";

function detectOS(): OS {
  if (typeof navigator === "undefined") return "windows";
  const ua = navigator.userAgent.toLowerCase();
  if (ua.includes("win")) return "windows";
  if (ua.includes("mac")) return "macos";
  return "linux";
}

interface InstallCardProps {
  product: Product;
}

export function InstallCard({ product }: InstallCardProps) {
  const [detectedOS, setDetectedOS] = useState<OS>("windows");

  useEffect(() => {
    setDetectedOS(detectOS());
  }, []);

  return (
    <section className="mt-8">
      <h2 className="text-xl font-semibold mb-4">Install</h2>
      <p className="text-sm text-gray-500 mb-4">
        Detected OS: <span className="font-medium capitalize">{detectedOS}</span>
        {" — "}showing matching tab first. Switch tabs for other platforms.
      </p>
      <OSToggle
        commands={product.install_commands}
        defaultOS={detectedOS}
      />
      <div className="mt-6">
        <a
          href={product.releases_url}
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex items-center gap-2 px-5 py-2.5 rounded-lg bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium transition-colors"
        >
          Download installer from GitHub Releases
        </a>
      </div>
    </section>
  );
}
