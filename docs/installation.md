# Install and uninstall

This candidate is not published. Public npm, crates.io and Homebrew installation commands are intentionally unavailable. Package-name availability does not establish ownership.

## One-line installer

The endpoint is prepared but not published:

~~~sh
curl -fsSL https://github.com/zhugez/traceveil/raw/main/install.sh | sh
~~~

The generated installer pins its release version and archive SHA256. It checks Linux/WSL x86_64 and the binary's measured glibc requirement before downloading the archive. It verifies archive bytes before extracting or executing the binary. No sudo, Rust, Python or Node.js is required for installation. Standard curl, tar and checksum utilities are required.

The default destination is HOME/.local/bin. Shell profiles remain unchanged. Follow the printed PATH guidance if that directory is absent from PATH.

Piping the script to sh executes code controlled by this repository. Trust the repository or review the script first. The embedded checksum binds archive bytes to that script. It is not independent publisher authentication. An empty or failed curl stream can leave sh returning zero. Check the download error and installed binary rather than relying only on the pipeline exit code.

For review or custom options, download the script to a file, inspect it, and invoke sh install.sh. --prefix, --force and --uninstall retain the manual installer safeguards. Public download validation remains a release gate.

## Source checkout

From a checkout containing this candidate:

~~~sh
cargo install --path . --locked
traceveil --version
traceveil demo
~~~

Building requires Rust and build dependencies. The installed binary needs no Rust, Python, Node.js or network connection. Cargo installation can use a custom --root PREFIX. Add PREFIX/bin to PATH. Uninstall with cargo uninstall traceveil, using the same --root when applicable.

## Locally prepared native archive

Build distribution files from the candidate checkout:

~~~sh
python3 scripts/package.py
(cd target/dist && sha256sum -c SHA256SUMS)
~~~

Use the native archive hash from target/dist/manifest.json when invoking the installer. Archive names include version and native platform. Linux archive preparation currently requires glibc. Local preparation does not publish anything.

On Linux, substitute the actual archive SHA256 for SHA256:

~~~sh
sh scripts/install.sh --version 0.2.0-rc.1 --archive target/dist/traceveil-0.2.0-rc.1-linux-x86_64.tar.gz --sha256 SHA256
traceveil demo
~~~

The default prefix is HOME/.local. TRACEVEIL_HOME or --prefix selects another absolute prefix. The installer prints the bin directory and does not change shell startup files. For the default prefix:

~~~sh
export PATH="$HOME/.local/bin:$PATH"
~~~

An explicit --version and independently trusted --sha256 can enable a versioned HTTPS release download after a release exists. The manual mode performs no latest lookup. The one-line mode explicitly executes the downloaded repository script. That remote path is not available or validated for this unreleased candidate.

Checksums detect corruption. They are not signed provenance or proof of publisher identity.

## Updates and removal

The installer records the installed binary hash. A replacement needs --force and an unchanged, receipted installation. Modified binaries and unowned files are preserved, even with --force. Failed checksum verification leaves the installation untouched.

~~~sh
sh scripts/install.sh --uninstall
~~~

Use the same --prefix for a custom installation. Uninstall removes the owned binary and receipt. It preserves user configuration, agent skills and unrelated files. Remove an optional skill separately:

~~~sh
traceveil skills uninstall --agent pi --scope project
~~~

Perform skill removal before binary removal if you want both. Traceveil does not automatically delete configuration or session data.

## Local npm tarball

The generated Linux x86_64/glibc .npm.tgz package is private. It links directly to the native executable and contains no dependencies, install scripts or downloader. npm is required for installation, not for running the installed binary.

~~~text
npm install --offline --ignore-scripts --no-audit --no-fund --prefix PREFIX ARTIFACT.npm.tgz
npm uninstall --prefix PREFIX traceveil
~~~

Use PREFIX/node_modules/.bin/traceveil. Linux/glibc installation is tested locally. A universal public npm distribution is not implemented or claimed.

## Windows and macOS

The packaging script can prepare native archives on those hosts. scripts/install.ps1 prepares explicit version/checksum installation and uninstall for PowerShell 7 on Windows. These are native validation targets, not current runtime support claims.

The Linux-generated Homebrew formula is a preparation artifact under target/release-prep. Homebrew and PowerShell are absent from the local validation host. See [release gates](releasing.md) for exact remaining checks.
