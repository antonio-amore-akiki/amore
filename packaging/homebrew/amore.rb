class Amore < Formula
  desc "Local-first MCP memory backbone for AI coding assistants — Apache-2.0, Rust, zero-cloud"
  homepage "https://github.com/antonio-amore-akiki/amore"
  version "0.5.0"
  license "Apache-2.0"

  on_macos do
    url "https://github.com/antonio-amore-akiki/amore/releases/download/v#{version}/amore-v#{version}-x86_64-apple-darwin.tar.gz"
    sha256 "PLACEHOLDER_macos_sha256"
  end

  on_linux do
    url "https://github.com/antonio-amore-akiki/amore/releases/download/v#{version}/amore-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "PLACEHOLDER_linux_sha256"
  end

  depends_on "qdrant" => :recommended
  depends_on "ollama" => :recommended

  def install
    bin.install "amore", "amore-mcp", "amore-gui"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/amore --version")
  end
end
