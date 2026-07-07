# Homebrew formula for saltc — the Salt compiler.
# To use: brew install --formula scripts/saltc.rb
# To publish: submit to homebrew-core or host in a tap (bneb/homebrew-salt).
class Saltc < Formula
  desc "Systems language compiler with Z3-powered compile-time verification"
  homepage "https://github.com/bneb/lattice"
  url "https://github.com/bneb/lattice/releases/download/v1.2.0/saltc-v1.2.0-darwin-arm64.tar.gz"
  sha256 "83863a95d2fb19dc52d50def1618a14b02d325fbf9aa1b468640b2bf1c84f933"
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
