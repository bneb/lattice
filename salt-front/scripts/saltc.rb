# Homebrew formula for saltc — the Salt compiler.
# To use: brew install --formula scripts/saltc.rb
# To publish: submit to homebrew-core or host in a tap (bneb/homebrew-salt).
class Saltc < Formula
  desc "Systems language compiler with Z3-powered compile-time verification"
  homepage "https://github.com/bneb/lattice"
  url "https://github.com/bneb/lattice/releases/download/v1.2.0/saltc-v1.2.0-macos-arm64.tar.gz"
  sha256 "REPLACE_WITH_ACTUAL_SHA256"
  version "1.2.0"
  license "MIT"

  depends_on "z3"

  def install
    bin.install "saltc"
  end

  test do
    (testpath/"test.salt").write <<~SALT
      package main
      pub fn main() -> i32 {
          let x: i32 = 42;
          return x;
      }
    SALT
    system "#{bin}/saltc", "test.salt", "--lib", "--disable-alias-scopes", "-o", "/dev/null"
  end
end
