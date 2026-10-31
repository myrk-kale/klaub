# Homebrew formula for Klaub.
#
# This file is meant to live in the tap repository myrk-kale/homebrew-klaub, at Formula/klaub.rb,
# so that `brew tap myrk-kale/klaub && brew install klaub` works on macOS and on Linux.
#
# The sha256 values are the digests of the v0.1.0 release assets.

class Klaub < Formula
  desc "Semantic code reviewer for humans that wraps any coding agent"
  homepage "https://github.com/myrk-kale/klaub"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/myrk-kale/klaub/releases/download/v0.1.0/klaub-0.1.0-aarch64-apple-darwin.tar.gz"
      sha256 "e5b8f468171b4a4dc1ee94876be5772aa42b250c34852edff5c4677487e0df27"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/myrk-kale/klaub/releases/download/v0.1.0/klaub-0.1.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "440b51fa1f3cc521c1131bd14ba4a096c88bea9bc5b1a6344442bc4884524ece"
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
