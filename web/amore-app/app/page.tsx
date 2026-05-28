import { getAllProducts } from "@/lib/products";
import { ProductGrid } from "@/components/ProductGrid";

export default function HomePage() {
  const products = getAllProducts();

  return (
    <div>
      <section className="text-center mb-16">
        <h1 className="text-4xl sm:text-5xl font-bold tracking-tight text-gray-900 mb-4">
          Amore — the app store for AI coding tools
        </h1>
        <p className="text-lg text-gray-600 max-w-2xl mx-auto">
          Discover, install, and manage tools that supercharge your AI-assisted development.
          Every tool is vetted, versioned, and published to all major package registries.
        </p>
      </section>

      <section>
        <h2 className="text-xl font-semibold text-gray-800 mb-6">
          Available tools ({products.length})
        </h2>
        <ProductGrid products={products} />
      </section>
    </div>
  );
}
