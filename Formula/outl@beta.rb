# This file is maintained by `.github/workflows/release.yml`.
# Every push to `main` runs the release workflow, which bumps the
# `version` line (computed from `Cargo.toml` + the workflow run
# number) and the three `sha256` lines below in place. The `# anchor:`
# comments are how the workflow finds the right lines — do not remove
# them.
#
# Values committed here are bootstrap placeholders: `version "0.0.0"`
# and zeroed SHAs make `brew install outl@beta` fail loudly until the
# first release fires. They become real on the next push to `main`.
class OutlATBeta < Formula
  desc "Local-first outliner with CRDT sync (beta channel — every push to main)"
  homepage "https://outl.app"
  version "0.6.0-beta.49"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/avelino/outl/releases/download/v#{version}/outl-macos-arm64.tar.gz"
      sha256 "edd98f9d05ea1786f5ba3ecdff556f922063be5fc0d534e5f4c69f24ac39ba89" # anchor: macos-arm64
    end
    on_intel do
      url "https://github.com/avelino/outl/releases/download/v#{version}/outl-macos-x64.tar.gz"
      sha256 "40ac0abdcab29a5bcb6bec4a8d18e412e7f94d675fef496a12187aa39c48a45c" # anchor: macos-x64
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/avelino/outl/releases/download/v#{version}/outl-linux-x64.tar.gz"
      sha256 "4524aa241921ef3cf3916e5b2c54cf113b439ed4a6beb9d8c4835f15557cc493" # anchor: linux-x64
    end
  end

  # Beta and stable share the same `outl` binary name. Refuse to install
  # both side-by-side — `brew unlink outl` (or `outl@beta`) before
  # switching channels.
  conflicts_with "outl", because: "both install the `outl` binary"

  def install
    bin.install "outl"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/outl --version")
  end
end
