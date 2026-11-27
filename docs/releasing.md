# Prepare a release

The candidate version is 0.2.0-rc.1. Preparation is local. This work does not authorize a push, public package, tag, tap update, GitHub release or deployment.

## Local preparation

~~~sh
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo build --release --locked
python3 scripts/package.py
python3 scripts/check_install.py
python3 scripts/check_release.py
python3 scripts/prepare_release.py
~~~

Install jsonschema in an isolated development environment before running scripts/check_contracts.py. Run scripts/check_terminal.py against the packaged/installed executable on a POSIX host. See CONTRIBUTING.md for the complete candidate checks.

Native archives use fixed ordering, ownership, modes and timestamps. The local npm tarball has no installation scripts. Verify reproducibility on the same host rather than assuming byte identity across different compilers or packaging tool versions.

The manual release-candidate workflow builds and uploads candidate workflow artifacts. It has read-only repository permissions and contains no publish or release command. Artifact access follows repository and CI policy. Running remote CI requires an authorized pushed source tree.

## External gates

| Gate | Current status |
|---|---|
| Source push | Not authorized for this overhaul |
| Version tag / public release | Not created; needs separate authorization |
| npm ownership and publication | Not verified; generated package remains private |
| crates.io ownership and publication | Not verified; only source installation/package preparation is exercised |
| Homebrew tap | No verified tap; generated Linux formula needs brew audit/install/test |
| Native macOS / Windows | Compile checks pass; native runtime and installer checks must run |
| Remote download install | No release asset exists for this candidate |
| Maintainer signing | Optional explicit full GPG fingerprint; tested with an isolated disposable key only. No maintainer key was supplied or used |
| Additional adapters | Require versioned fixtures and authorized real-session evidence |

Before publishing, verify package ownership and licenses, rerun dependency/security checks, execute the installed-user journey on each advertised platform and inspect current-tree receipts. Keep raw real sessions local. Historical v0.1 validation is not evidence for this candidate binary.

## Promote the one-line installer

prepare_release.py generates target/release-prep/install.sh for Linux x86_64. It uses scripts/install.sh as its sole maintained implementation. Version, native archive SHA256 and the measured glibc requirement are embedded. The generated script delays side effects until its complete function body is parsed. This does not authenticate downloaded code or make a failed curl pipeline reliable.

After the archive is approved, copy that exact build's generated installer to repository-root install.sh. Publish the exact native archive bytes referenced by the script. A later CI rebuild can have another checksum. Do not promote an installer from one build alongside another build's archive. The generated installer stays outside its payload to avoid a checksum cycle.

The documented raw/main one-line URL becomes usable only after the root script and pinned release asset are both published. This preparation does not authorize a commit, push or release.

## Optional local signing

prepare_release.py records checksums and self-reported local build evidence in provenance.json. package.py records source hashes before and after the Cargo build. It selects the executable from Cargo output. --skip-build explicitly leaves source binding unverified. This evidence is not an independent builder attestation or a SLSA level claim.

To sign with a separately approved maintainer key, pass --sign-key followed by its full hexadecimal fingerprint. The tool never chooses a default key. GPG is optional for preparation and required for this explicit signing command. Verify the detached provenance.json.asc with a separately trusted public key. A signature authenticates the statement to that key. It does not establish namespace ownership or prove the statement independently.

check_release.py tests signing and tamper rejection in a temporary GPG home. It removes the temporary key. Normal candidate artifacts remain unsigned.

## Evidence

Current candidate receipts belong under benchmarks/results/productization*.json and their associated check logs. Preserve historical preview manifests and receipts. A report must identify its source inventory, binary checksum, platform, actual commands, exit status and log hashes.

A maintainer can review target/release-prep/RELEASE_NOTES.md and traceveil.rb after running prepare_release.py. The formula targets only the checksummed Linux x86_64 artifact. It is not a published installation channel.
