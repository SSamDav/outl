# This file is maintained by `.github/workflows/release.yml`.
# Every push to `main` runs the release workflow, which bumps the
# `version` line (computed from `Cargo.toml` + the workflow run
# number) and the three `sha256` lines below in place. The `# anchor:`
# comments are how the workflow finds the right lines — do not remove
# them.
#
# Values committed here are bootstrap placeholders: `version "0.0.0"`
# and zeroed SHAs make `brew install outl-beta` fail loudly until the
# first release fires. They become real on the next push to `main`.
class OutlBeta < Formula
  desc "Local-first outliner with CRDT sync (beta channel — every push to main)"
  homepage "https://outl.app"
  version "0.8.0-beta.110"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/avelino/outl/releases/download/v#{version}/outl-macos-arm64.tar.gz"
      sha256 "483f0ccec7c03b9a1c6ae45b9d62787a90f459cc57f32fde19b8701543455086" # anchor: macos-arm64
    end
    on_intel do
      url "https://github.com/avelino/outl/releases/download/v#{version}/outl-macos-x64.tar.gz"
      sha256 "fdeaa0a03064cfb16b366a6cef67b9de60478dd333b7f7a0c6732a9224ade6d9" # anchor: macos-x64
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/avelino/outl/releases/download/v#{version}/outl-linux-x64.tar.gz"
      sha256 "5e3d1cd8c078641d0092277f0124c2961256cd3a27fad622af4afd76abd38036" # anchor: linux-x64
    end
  end

  # Beta and stable share the same `outl` binary name. Refuse to install
  # both side-by-side — `brew unlink outl` (or `outl-beta`) before
  # switching channels.
  conflicts_with "outl", because: "both install the `outl` binary"

  def install
    bin.install "outl"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/outl --version")
  end
end
