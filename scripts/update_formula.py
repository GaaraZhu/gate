#!/usr/bin/env python3
"""Regenerate Formula/gate.rb with multi-arch url/sha256 blocks.

Usage:
  python3 update_formula.py <version> <sha_macos_arm> <sha_macos_intel> \
                             <sha_linux_arm> <sha_linux_intel>
"""
import sys

version, sha_macos_arm, sha_macos_intel, sha_linux_arm, sha_linux_intel = sys.argv[1:6]
base = f"https://github.com/GaaraZhu/gate/releases/download/v{version}"

formula = f"""\
class Gate < Formula
  desc "PII-filtering CLI that intercepts AI agent query results and redacts sensitive data"
  homepage "https://github.com/GaaraZhu/gate"
  license "MIT"
  version "{version}"

  on_macos do
    on_arm do
      url "{base}/gate-{version}-aarch64-apple-darwin.tar.gz"
      sha256 "{sha_macos_arm}"
    end
    on_intel do
      url "{base}/gate-{version}-x86_64-apple-darwin.tar.gz"
      sha256 "{sha_macos_intel}"
    end
  end

  on_linux do
    on_arm do
      url "{base}/gate-{version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "{sha_linux_arm}"
    end
    on_intel do
      url "{base}/gate-{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "{sha_linux_intel}"
    end
  end

  def install
    bin.install "gate"
  end

  test do
    assert_match version.to_s, shell_output("\#{bin}/gate version")
  end
end
"""

with open("Formula/gate.rb", "w") as f:
    f.write(formula)

print(f"Updated Formula/gate.rb to v{version}")
