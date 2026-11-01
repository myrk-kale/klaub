# Homebrew formula for Klaub.
#
# This file lives in the shared tap myrk-kale/homebrew-tap, at Formula/klaub.rb, so that
# `brew install myrk-kale/tap/klaub` works on macOS and on Linux. The older tap
# myrk-kale/homebrew-klaub keeps a copy for people who already added it.
#
# The sha256 values are the digests of the v0.4.0 release assets.

class Klaub < Formula
  desc "Terminal code reviewer for the human reading AI-written code"
  homepage "https://github.com/myrk-kale/klaub"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/myrk-kale/klaub/releases/download/v0.4.0/klaub-0.4.0-aarch64-apple-darwin.tar.gz"
      sha256 "45c84d7ff46024ace052126b57f9a45eba7ca0440fd105945cb1d78e75201fa4"
    end
    on_intel do
      url "https://github.com/myrk-kale/klaub/releases/download/v0.4.0/klaub-0.4.0-x86_64-apple-darwin.tar.gz"
      sha256 "8e3c4a1d0dddbaf6f9626c985956228020267ca4f728b21279e03abdd7361bb0"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/myrk-kale/klaub/releases/download/v0.4.0/klaub-0.4.0-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "8a01abdccbfd21e2d27d93e5db7e02e85abb2d22982de4de0cdfe52bf2668bca"
    end
    on_intel do
      url "https://github.com/myrk-kale/klaub/releases/download/v0.4.0/klaub-0.4.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "4135474766ac023879bda2bd5a925d6d2fa2597204cbdcd94f28e34271320458"
    end
  end

  def install
    bin.install "klaub"
    prefix.install "LICENSE", "README.md", "CHANGELOG.md", "THIRD-PARTY-NOTICES.md"
    pkgshare.install "licenses", "dependencies.txt"
  end

  test do
    assert_match "klaub 0.4.0", shell_output("#{bin}/klaub version")
  end
end
