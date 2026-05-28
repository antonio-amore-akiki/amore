import type { Metadata } from "next";
import Link from "next/link";
import "./globals.css";

const SITE_URL = "https://antonio-amore-akiki.github.io/amore";
const TITLE = "Amore — the app store for AI coding tools";
const DESCRIPTION =
  "Discover and install AI coding tools. Amore is the local-first MCP memory backbone for Claude Code, Cursor, and other AI IDEs.";

export const metadata: Metadata = {
  metadataBase: new URL(SITE_URL),
  title: { default: TITLE, template: "%s — Amore" },
  description: DESCRIPTION,
  applicationName: "Amore",
  authors: [{ name: "Antonio Amore Akiki", url: "https://github.com/antonio-amore-akiki" }],
  keywords: [
    "mcp",
    "agent-memory",
    "claude-code",
    "cursor",
    "ai-coding-tools",
    "local-first",
    "amore",
  ],
  openGraph: {
    type: "website",
    url: SITE_URL,
    siteName: "Amore",
    title: TITLE,
    description: DESCRIPTION,
    locale: "en_US",
  },
  twitter: {
    card: "summary_large_image",
    title: TITLE,
    description: DESCRIPTION,
  },
  icons: {
    icon: [{ url: "/amore/favicon.svg", type: "image/svg+xml" }],
    shortcut: "/amore/favicon.svg",
  },
  robots: { index: true, follow: true },
};

interface RootLayoutProps {
  children: React.ReactNode;
}

export default function RootLayout({ children }: RootLayoutProps) {
  return (
    <html lang="en">
      <body className="min-h-screen bg-gray-50 text-gray-900 antialiased">
        <header className="border-b border-gray-200 bg-white">
          <div className="max-w-6xl mx-auto px-4 sm:px-6 h-16 flex items-center justify-between">
            <Link href="/" className="text-lg font-bold tracking-tight text-gray-900 hover:text-blue-600 transition-colors">
              Amore
            </Link>
            <nav className="flex items-center gap-6 text-sm">
              <Link href="/" className="text-gray-600 hover:text-gray-900">Catalog</Link>
              <Link href="/about" className="text-gray-600 hover:text-gray-900">About</Link>
              <Link href="/legal" className="text-gray-600 hover:text-gray-900">Legal</Link>
            </nav>
          </div>
        </header>
        <main className="max-w-6xl mx-auto px-4 sm:px-6 py-12">
          {children}
        </main>
        <footer className="border-t border-gray-200 mt-16">
          <div className="max-w-6xl mx-auto px-4 sm:px-6 py-8 text-sm text-gray-500 flex flex-col sm:flex-row items-center justify-between gap-4">
            <span>&copy; {new Date().getFullYear()} Amore. All rights reserved.</span>
            <div className="flex gap-6">
              <Link href="/legal" className="hover:text-gray-700">Legal</Link>
              <a href="https://github.com/antonio-amore-akiki/amore" target="_blank" rel="noopener noreferrer" className="hover:text-gray-700">GitHub</a>
            </div>
          </div>
        </footer>
      </body>
    </html>
  );
}
