import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

export type InstallCommand = {
  channel: string;
  command: string;
  label: string;
  type: "shell" | "gui";
};

export type Product = {
  slug: string;
  name: string;
  description: string;
  github_url: string;
  latest_version: string;
  install_commands: {
    windows: InstallCommand[];
    macos: InstallCommand[];
    linux: InstallCommand[];
  };
  releases_url: string;
};

const PRODUCTS_DIR = join(process.cwd(), "content", "products");

export function getAllProducts(): Product[] {
  const files = readdirSync(PRODUCTS_DIR).filter((f) => f.endsWith(".json"));
  return files.map((file) => {
    const raw = readFileSync(join(PRODUCTS_DIR, file), "utf-8");
    return JSON.parse(raw) as Product;
  });
}

export function getProductBySlug(slug: string): Product | undefined {
  const all = getAllProducts();
  return all.find((p) => p.slug === slug);
}

export function getAllSlugs(): string[] {
  return getAllProducts().map((p) => p.slug);
}
