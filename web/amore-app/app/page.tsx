import { getAllProducts } from "@/lib/products";
import { ProductGrid } from "@/components/ProductGrid";

const LAUNCHER_RELEASE_URL =
  "https://github.com/antonio-amore-akiki/amore/releases/tag/launcher-v1.1.0";

export default function HomePage() {
  const products = getAllProducts();

  return (
    <div>
      <section className="text-center mb-16">
        <h1 className="text-4xl sm:text-5xl font-bold tracking-tight text-gray-900 mb-4">
          Amore — the app store for AI coding tools
        </h1>
        <p className="text-lg text-gray-600 max-w-2xl mx-auto mb-8">
          Discover, install, and manage tools that supercharge your AI-assisted development.
          Every tool is vetted, versioned, and published to all major package registries.
        </p>
        <div className="flex flex-col sm:flex-row gap-3 justify-center">
          <a
            href={LAUNCHER_RELEASE_URL}
            className="inline-flex items-center justify-center px-6 py-3 rounded-lg bg-blue-600 text-white font-semibold hover:bg-blue-700 transition-colors"
          >
            Download Amore launcher
          </a>
          <a
            href="#catalog"
            className="inline-flex items-center justify-center px-6 py-3 rounded-lg border border-gray-300 text-gray-700 font-semibold hover:bg-gray-100 transition-colors"
          >
            Browse catalog ↓
          </a>
        </div>
        <p className="text-xs text-gray-500 mt-4">
          Cross-platform (Windows, macOS, Linux) · Open source · Sigstore-signed
        </p>
      </section>

      <section id="catalog">
        <h2 className="text-xl font-semibold text-gray-800 mb-6">
          Available tools ({products.length})
        </h2>
        <ProductGrid products={products} />
      </section>
    </div>
  );
}
