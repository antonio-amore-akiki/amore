/** @type {import('next').NextConfig} */
const nextConfig = {
  // Static export for GitHub Pages (free + agent-deployable via gh-pages branch).
  // No backend dependency — product catalog is JSON at build time.
  output: "export",
  // GitHub Pages serves the repo at /amore subpath when published from gh-pages branch
  // on antonio-amore-akiki/amore (vs <username>.github.io which serves at root). Configure
  // basePath + assetPrefix so internal links resolve correctly under /amore/.
  basePath: "/amore",
  assetPrefix: "/amore",
  // Disable Next image optimization (requires Node runtime; static export uses raw <img>)
  images: { unoptimized: true },
  // Trailing slash so GH Pages serves /about/ -> /about/index.html
  trailingSlash: true,
};

export default nextConfig;
