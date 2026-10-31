# Homebrew formula for Klaub.
#
# This file is meant to live in the tap repository myrk-kale/homebrew-klaub, at Formula/klaub.rb,
# so that `brew tap myrk-kale/klaub && brew install klaub` works on macOS and on Linux.
#
# The sha256 values below are placeholders. A release asset does not exist until the v0.1.0 release
# is published, and a formula must pin the exact file it downloads. After `gh release create v0.1.0`
# finishes, replace each PLACEHOLDER with the digest from the matching .tar.gz.sha256 asset:
#
#   gh release download v0.1.0 --pattern '*.tar.gz.sha256' --dir /tmp/klaub-sums
#   cat /tmp/klaub-sums/*.sha256
#
# `brew audit --strict` will reject the formula until that is done, which is what we want.

class Klaub < Formula
  desc "Semantic code reviewer for humans that wraps any coding agent"
  homepage "https://github.com/myrk-kale/klaub"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/myrk-kale/klaub/releases/download/v0.1.0/klaub-0.1.0-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_AARCH64_APPLE_DARWIN"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/myrk-kale/klaub/releases/download/v0.1.0/klaub-0.1.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER_X86_64_LINUX_GNU"
    end
  end

  def install
    bin.install "klaub"
    prefix.install "LICENSE", "README.md", "CHANGELOG.md", "THIRD-PARTY-NOTICES.md"
    pkgshare.install "licenses", "dependencies.txt"
  end

  test do
    assert_match "klaub 0.1.0", shell_output("#{bin}/klaub version")
  end
end
