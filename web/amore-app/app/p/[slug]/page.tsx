import { notFound } from "next/navigation";
import Link from "next/link";
import { getAllSlugs, getProductBySlug } from "@/lib/products";
import { InstallCard } from "@/components/InstallCard";
import type { Metadata } from "next";

interface PageParams {
  slug: string;
}

interface PageProps {
  params: Promise<PageParams>;
}

export async function generateStaticParams(): Promise<PageParams[]> {
  return getAllSlugs().map((slug) => ({ slug }));
}

export async function generateMetadata({ params }: PageProps): Promise<Metadata> {
  const { slug } = await params;
  const product = getProductBySlug(slug);
  if (!product) return { title: "Not found" };
  return {
    title: `${product.name} — Amore`,
    description: product.description,
  };
}

export default async function ProductPage({ params }: PageProps) {
  const { slug } = await params;
  const product = getProductBySlug(slug);

  if (!product) {
    notFound();
  }

  return (
    <div className="max-w-3xl">
      <div className="mb-2">
        <Link href="/" className="text-sm text-gray-500 hover:text-gray-700">
          ← Back to catalog
        </Link>
      </div>

      <div className="text-5xl mt-4 mb-4">🧠</div>

      <h1 className="text-3xl font-bold text-gray-900 mb-3">{product.name}</h1>

      <div className="flex items-center gap-3 mb-6">
        <span className="text-xs font-mono bg-gray-100 text-gray-600 px-2 py-1 rounded">
          v{product.latest_version}
        </span>
        <a
          href={product.github_url}
          target="_blank"
          rel="noopener noreferrer"
          className="text-xs text-blue-600 hover:text-blue-800 font-medium"
        >
          GitHub
        </a>
      </div>

      <p className="text-gray-700 leading-relaxed mb-8">{product.description}</p>

      <InstallCard product={product} />
    </div>
  );
}
