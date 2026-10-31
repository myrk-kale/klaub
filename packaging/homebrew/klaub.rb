# Homebrew formula for Klaub.
#
# This file is meant to live in the tap repository myrk-kale/homebrew-klaub, at Formula/klaub.rb,
# so that `brew tap myrk-kale/klaub && brew install klaub` works on macOS and on Linux.
#
# The sha256 values are the digests of the v0.2.0 release assets.

class Klaub < Formula
  desc "Semantic code reviewer for humans that wraps any coding agent"
  homepage "https://github.com/myrk-kale/klaub"
  version "0.2.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/myrk-kale/klaub/releases/download/v0.2.0/klaub-0.2.0-aarch64-apple-darwin.tar.gz"
      sha256 "ba7d09d5ce29b90e8820e25f9e99fdfe1ed57a3c029b32abb06947ad02aadd99"
    end
    on_intel do
      url "https://github.com/myrk-kale/klaub/releases/download/v0.2.0/klaub-0.2.0-x86_64-apple-darwin.tar.gz"
      sha256 "f53b7eb87c7b4a5afda164b363ad138e7a1eebeeb24d07620228f6b8394554df"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/myrk-kale/klaub/releases/download/v0.2.0/klaub-0.2.0-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "f09a984493d9090f9974542f6a1c42de36cbb7b5f56911ca35cf4f9b98e8c3e0"
    end
    on_intel do
      url "https://github.com/myrk-kale/klaub/releases/download/v0.2.0/klaub-0.2.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "8e70a7eaf2d7105cf2aeaf006af3045624435492e738f2306486700167758291"
    end
  end

  def install
    bin.install "klaub"
    prefix.install "LICENSE", "README.md", "CHANGELOG.md", "THIRD-PARTY-NOTICES.md"
    pkgshare.install "licenses", "dependencies.txt"
  end

  test do
    assert_match "klaub 0.2.0", shell_output("#{bin}/klaub version")
  end
end
