# Homebrew formula for Klaub.
#
# This file lives in the shared tap myrk-kale/homebrew-tap, at Formula/klaub.rb, so that
# `brew install myrk-kale/tap/klaub` works on macOS and on Linux. The older tap
# myrk-kale/homebrew-klaub keeps a copy for people who already added it.
#
# The sha256 values are the digests of the v0.3.1 release assets.

class Klaub < Formula
  desc "Terminal code reviewer for the human reading AI-written code"
  homepage "https://github.com/myrk-kale/klaub"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/myrk-kale/klaub/releases/download/v0.3.1/klaub-0.3.1-aarch64-apple-darwin.tar.gz"
      sha256 "6ebe71f7ffc2a93820c3d113733c1784187e62f4e0fdb678a362b7eb8fa48e48"
    end
    on_intel do
      url "https://github.com/myrk-kale/klaub/releases/download/v0.3.1/klaub-0.3.1-x86_64-apple-darwin.tar.gz"
      sha256 "37a502d615c08e1da86ea4e27b35ae7d5df799a874b4faa68ce3c33185a90b0d"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/myrk-kale/klaub/releases/download/v0.3.1/klaub-0.3.1-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "e4b3cf65f3b387f9079074c0165264914f54e5b2a2d1b16b0e5bdfdb1218e2ac"
    end
    on_intel do
      url "https://github.com/myrk-kale/klaub/releases/download/v0.3.1/klaub-0.3.1-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "bc16847b8d88a254c664e3ed72f219c1a7bb1b808b5b299f9736a1549b5ca3af"
    end
  end

  def install
    bin.install "klaub"
    prefix.install "LICENSE", "README.md", "CHANGELOG.md", "THIRD-PARTY-NOTICES.md"
    pkgshare.install "licenses", "dependencies.txt"
  end

  test do
    assert_match "klaub 0.3.1", shell_output("#{bin}/klaub version")
  end
end
