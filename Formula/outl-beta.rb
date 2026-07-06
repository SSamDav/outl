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
  version "0.7.0-beta.102"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/avelino/outl/releases/download/v#{version}/outl-macos-arm64.tar.gz"
      sha256 "72f9f72e675147937b9ce16767d6e24adab07d501ecacee0928b4052c0c40b3f" # anchor: macos-arm64
    end
    on_intel do
      url "https://github.com/avelino/outl/releases/download/v#{version}/outl-macos-x64.tar.gz"
      sha256 "c75ceae88de2d2cb614861d607474baddf8969b731d8650fd7cbf1b0b8bc4d14" # anchor: macos-x64
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/avelino/outl/releases/download/v#{version}/outl-linux-x64.tar.gz"
      sha256 "6eec2f58d52941eac227a2ab6fff82ba5b47357246163a9cd1ec9c44ed820b5d" # anchor: linux-x64
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
