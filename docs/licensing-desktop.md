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
dylibs; matching Homebrew source archives, recipes and notices are retained.

Experimental prerelease packaging collects notices and exact source archives for
Rust runtime/build dependencies, with pinned upstream-commit recovery when crate
archives omit license files. Linux library sources retain distributor .dsc files
and patches; macOS libraries retain matching Homebrew sources and recipes. Unknown
provenance or missing notices stop the build. Rust sources/inventories are release
assets; bundled engine/library sources accompany the private payload.

This is a reproducible-notice/source retention check, not independent legal advice
or a supply-chain audit. Developer-ID signing/notarization remain absent, and the
public release notes must state experimental status and functional limitations.
Publishing occurs only through the explicitly requested prerelease job, after all
package and Linux review gates pass; ordinary review pushes do not publish.

Pinned objc2 revisions retain their upstream MIT summary and original full
Steven Sheldon notice from the documented license-clarification parent commit.
For dispatch 0.2.0, the declared MIT license and author are checked exactly,
and a reconstructed MIT notice is explicitly identified as such. Other
unreviewed notice omissions still fail closed. See packaging/licenses for
provenance; no independent legal audit is asserted.

The public README displays the official Slint attribution badge, retained from
v1.19.0, in addition to the AboutSlint widget reached from About / help. This
preserves the badge attribution option without an extra in-window menu bar.
The notice collector recognizes UNLICENSE files, including ksni.
