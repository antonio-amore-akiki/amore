import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "About — Amore",
  description: "About Amore: the app store for AI coding tools, and amore: the local-first MCP memory backbone.",
};

export default function AboutPage() {
  return (
    <div className="max-w-2xl">
      <h1 className="text-3xl font-bold text-gray-900 mb-8">About</h1>

      <section className="mb-10">
        <h2 className="text-xl font-semibold text-gray-800 mb-4">
          Amore — the platform
        </h2>
        <p className="text-gray-700 leading-relaxed mb-4">
          <strong>Amore</strong> (capital A) is an open-source app store for AI coding tools.
          It is a curated catalog where developers can discover, install, and keep AI-powered
          development tools up to date — across all major package registries and operating systems.
        </p>
        <p className="text-gray-700 leading-relaxed">
          Every tool listed on Amore is published to cargo, npm, PyPI, Docker Hub, and GitHub
          Releases simultaneously, with cosign keyless OIDC signatures for supply-chain
          verification. Amore is built for engineers who take reproducibility and security
          seriously.
        </p>
      </section>

      <section className="mb-10">
        <h2 className="text-xl font-semibold text-gray-800 mb-4">
          amore — the MCP memory backbone
        </h2>
        <p className="text-gray-700 leading-relaxed mb-4">
          <strong>amore</strong> (lowercase) is the first tool in the catalog. It is a
          local-first Model Context Protocol (MCP) server that gives Claude Code, Cursor,
          Cline, Continue, and other MCP-aware IDEs a persistent, searchable memory layer.
        </p>
        <p className="text-gray-700 leading-relaxed mb-4">
          With amore, your AI assistant can <em>observe</em> facts during a session,{" "}
          <em>recall</em> them across sessions, and <em>forget</em> outdated ones — all
          stored locally, never sent to a third party.
        </p>
        <p className="text-gray-700 leading-relaxed">
          amore is built in Rust for performance and reliability. It runs as a lightweight
          background process and exposes three MCP tools: <code className="text-sm bg-gray-100 px-1 rounded">observe</code>,{" "}
          <code className="text-sm bg-gray-100 px-1 rounded">recall</code>, and{" "}
          <code className="text-sm bg-gray-100 px-1 rounded">forget</code>.
        </p>
      </section>

      <section>
        <h2 className="text-xl font-semibold text-gray-800 mb-4">Source</h2>
        <p className="text-gray-700 leading-relaxed">
          Both Amore and amore are open source under the MIT license. Source code and
          issue tracker:{" "}
          <a
            href="https://github.com/antonio-amore-akiki/amore"
            target="_blank"
            rel="noopener noreferrer"
            className="text-blue-600 hover:text-blue-800 underline"
          >
            github.com/antonio-amore-akiki/amore
          </a>
          .
        </p>
      </section>
    </div>
  );
}
