import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Legal — Amore",
  description: "Terms of Service and Privacy Policy for Amore.",
};

export default function LegalPage() {
  return (
    <div className="max-w-2xl">
      <h1 className="text-3xl font-bold text-gray-900 mb-8">Legal</h1>

      <section className="mb-12">
        <h2 className="text-xl font-semibold text-gray-800 mb-4">Terms of Service</h2>
        <div className="prose prose-gray text-gray-700 space-y-4 leading-relaxed">
          <p>
            By using Amore or any tool published through the Amore catalog, you agree to
            the following terms. These terms are a stub for Phase α MVP and will be
            expanded before public launch.
          </p>
          <p>
            <strong>Use at your own risk.</strong> Amore is provided &ldquo;as is&rdquo; without warranty
            of any kind. Tools in the catalog are open-source software; their individual
            licenses govern their use.
          </p>
          <p>
            <strong>No warranty.</strong> The author makes no representations about the
            suitability, reliability, availability, timeliness, or accuracy of the software
            or information available through Amore.
          </p>
          <p>
            <strong>Limitation of liability.</strong> In no event shall the author be liable
            for any damages arising from the use or inability to use Amore or any tool in
            its catalog.
          </p>
          <p className="text-sm text-gray-500 italic">
            Last updated: May 2026. Full legal review pending before public GA.
          </p>
        </div>
      </section>

      <section>
        <h2 className="text-xl font-semibold text-gray-800 mb-4">Privacy Policy</h2>
        <div className="prose prose-gray text-gray-700 space-y-4 leading-relaxed">
          <p>
            <strong>No personal data collected.</strong> Amore (the website) does not collect,
            store, or transmit any personal information. There are no accounts, no cookies,
            no analytics trackers, and no third-party SDKs in the Phase α MVP.
          </p>
          <p>
            <strong>amore (the tool) is local-first.</strong> All memory stored by the amore
            MCP server lives entirely on your machine. No data is sent to any server operated
            by the author or any third party.
          </p>
          <p>
            <strong>GitHub.</strong> This site links to GitHub (github.com/antonio-amore-akiki/amore).
            GitHub&apos;s own privacy policy applies to any interaction on their platform.
          </p>
          <p className="text-sm text-gray-500 italic">
            Last updated: May 2026. Full legal review pending before public GA.
          </p>
        </div>
      </section>
    </div>
  );
}
