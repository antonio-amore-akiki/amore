import type { MetadataRoute } from "next";
import { getAllSlugs } from "@/lib/products";

const BASE = "https://antonio-amore-akiki.github.io/amore";

export default function sitemap(): MetadataRoute.Sitemap {
  const now = new Date();
  const staticRoutes = ["", "/about", "/legal"].map((path) => ({
    url: `${BASE}${path}/`,
    lastModified: now,
    changeFrequency: "weekly" as const,
    priority: path === "" ? 1.0 : 0.6,
  }));
  const productRoutes = getAllSlugs().map((slug) => ({
    url: `${BASE}/p/${slug}/`,
    lastModified: now,
    changeFrequency: "weekly" as const,
    priority: 0.8,
  }));
  return [...staticRoutes, ...productRoutes];
}
