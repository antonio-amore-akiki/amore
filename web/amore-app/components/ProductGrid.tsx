import Link from "next/link";
import type { Product } from "@/lib/products";

interface ProductCardProps {
  product: Product;
}

function ProductCard({ product }: ProductCardProps) {
  return (
    <Link
      href={`/p/${product.slug}`}
      className="block p-6 rounded-xl border border-gray-200 hover:border-blue-400 hover:shadow-md transition-all bg-white"
    >
      <div className="text-3xl mb-3">🧠</div>
      <h2 className="text-lg font-semibold text-gray-900 mb-2">{product.name}</h2>
      <p className="text-sm text-gray-600 line-clamp-3">{product.description}</p>
      <div className="mt-4 flex items-center justify-between">
        <span className="text-xs font-mono text-gray-400">v{product.latest_version}</span>
        <span className="text-xs text-blue-600 font-medium">View →</span>
      </div>
    </Link>
  );
}

interface ProductGridProps {
  products: Product[];
}

export function ProductGrid({ products }: ProductGridProps) {
  if (products.length === 0) {
    return (
      <p className="text-gray-500 text-sm">No products found.</p>
    );
  }

  return (
    <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-6">
      {products.map((product) => (
        <ProductCard key={product.slug} product={product} />
      ))}
    </div>
  );
}
