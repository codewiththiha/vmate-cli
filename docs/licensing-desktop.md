# Desktop and bundled dependency licensing

vmate's own source remains MIT. Slint is NOT MIT. The desktop frontend selects
Slint Royalty-free Desktop, Mobile, and Web Applications License 2.0, appropriate
for general-purpose desktop applications. The official AboutSlint widget appears
on Help → About vmate, satisfying the license's in-app attribution option. The
unchanged terms ship under packaging/licenses and inside installation packages.
This selection does not cover embedded systems or re-exposing Slint APIs.

Slint licensing: https://slint.dev/license
Source terms: https://github.com/slint-ui/slint/tree/v1.19.0/LICENSES
The external review branch is not a representation of legal approval.

Bundled OpenVPN is built from the SHA256-pinned upstream 2.7.8 source. Its archive,
COPYING and source provenance ship with bundled editions. OpenVPN carries GPL
terms and an OpenSSL exception; inspect the exact included notices. Bundled
shared libraries have their own licenses. Linux packaging copies distributor
copyright files and records their final hashes. macOS dylibbundler copies runtime
dylibs; corresponding license/source offers still need a redistribution audit.

Release gates: cargo-about or equivalent dependency inventory; review all Slint,
OpenVPN, OpenSSL, LZO, LZ4 and bundled-library notices; provide any required
corresponding source/relinking material. Developer-ID signing and notarization
are separate requirements, not a substitute for license compliance.
No public GUI release should bypass these gates. Packaging CI creates review
artifacts only and never publishes a GitHub release.
