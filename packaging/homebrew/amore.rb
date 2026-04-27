class Amore < Formula
  desc "Local-first MCP memory backbone for AI coding assistants — Apache-2.0, Rust, zero-cloud"
  homepage "https://github.com/antonio-amore-akiki/amore"
  version "1.0.2"
  license "Apache-2.0"

  # macOS binaries built by .github/workflows/release.yml macos-build job on free
  # GHA macos-latest runners (post 2026-05-27 public-flip). Tarball naming matches
  # the workflow output: amore-<version>-macos-<arch_short>.tar.gz.
  on_macos do
    on_intel do
      url "https://github.com/antonio-amore-akiki/amore/releases/download/v1.0.0/amore-1.0.0-macos-x86_64.tar.gz"
      sha256 "0875d71e9880ceda8bdc0b59d00c976c0f04c98c1df88a3a357c19f88ca92723"
    end

    on_arm do
      url "https://github.com/antonio-amore-akiki/amore/releases/download/v1.0.0/amore-1.0.0-macos-aarch64.tar.gz"
      sha256 "198e17227e72175421d1b33c7296b6260d51e5d91510026204c2670fb3c3e988"
    end
  end

  on_linux do
    url "https://github.com/antonio-amore-akiki/amore/releases/download/v1.0.0/amore-gui-x86_64.AppImage"
    sha256 "3e433ae1bf09dfd6e35efef5400bf3e119eb73243d8cda589e1c739e4ce70597"
  end

  depends_on "qdrant" => :recommended
  depends_on "ollama" => :recommended

  def install
    on_macos do
      # macOS tarball contains amore + amore-mcp + amore-gui at top level
      bin.install "amore"
      bin.install "amore-mcp"
      bin.install "amore-gui"
    end
    on_linux do
      # Linux: AppImage is self-contained portable binary
      bin.install "amore-gui-x86_64.AppImage" => "amore-gui"
    end
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/amore-gui --version 2>&1")
  end
end
