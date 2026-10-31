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
      sha256 "25b4e34402c17a567d44aac935758d1491eec4f4fd973f011b3c8a2902b4bc40"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/myrk-kale/klaub/releases/download/v0.1.0/klaub-0.1.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "7dc6f8879d390bf1346e52c7761c67478157a301139f96c5c658d2044fc9c838"
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
