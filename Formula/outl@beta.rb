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
  version "0.6.0-beta.56"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/avelino/outl/releases/download/v#{version}/outl-macos-arm64.tar.gz"
      sha256 "c562a3e95f6ad0c28dba8f0416b9546efabebde2999c615efe75d05f69294b20" # anchor: macos-arm64
    end
    on_intel do
      url "https://github.com/avelino/outl/releases/download/v#{version}/outl-macos-x64.tar.gz"
      sha256 "017a77900c05cc54bcb115673feeb1d79e5db7868cdac3328c3c7ec4f3e70691" # anchor: macos-x64
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/avelino/outl/releases/download/v#{version}/outl-linux-x64.tar.gz"
      sha256 "6c5e5ed0de6eca2dfa774799ed7f4ccdee1b8a2a72a0d8e4a1bbe8255d70fdcd" # anchor: linux-x64
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
