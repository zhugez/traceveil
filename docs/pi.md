# Analyze Pi sessions

~~~sh
traceveil sessions --agent pi
traceveil analyze --last
traceveil analyze SESSION --agent pi
~~~

Discovery is non-recursive. It considers regular .jsonl files in one directory and reads bounded headers for metadata. It never follows parentSession or paths inside a session. A cwd header filters candidates; it does not authorize filesystem access.

## Directory selection

The product analyze path uses:

1. --session-dir, then the explicitly saved Traceveil session_dir.
2. PI_CODING_AGENT_SESSION_DIR.
3. Pi project .pi/settings.json sessionDir, then Pi user settings sessionDir.
4. Pi's encoded current-project directory below PI_CODING_AGENT_DIR/sessions or HOME/.pi/agent/sessions.

A project-controlled sessionDir requires interactive confirmation or an explicit --session-dir. --last alone does not trust a repository redirect. JSON mode never asks for consent. It returns DISCOVERY_AUTHORIZATION_REQUIRED instead.

Bare analyze asks before first discovery in a terminal. In pipelines, supply --last, a source file or --session-dir. init saves a user-authorized directory. No whole-home search occurs.

The compatibility diagnose --last and paths-only sessions --json retain the original directory behavior: explicit directory, PI_CODING_AGENT_SESSION_DIR, then the encoded default. They do not consume Pi settings. Add --agent pi or --metadata for the product metadata listing.

## Selection and limits

--last selects the newest modified matching Pi v3 file. Ties sort by path. Without --last, one candidate is selected directly. Several candidates open a terminal menu or produce SESSION_AMBIGUOUS in non-interactive mode.

At most 10,000 directory entries are enumerated. Metadata inspection considers the first 100 candidate files. Narrow the directory if your matching session is older than that window. Unsupported and malformed headers remain visible in metadata listings. They are not silently treated as supported adapters.

A missing final newline is labeled incomplete_tail. A recent modification is only a possibly_active hint. Neither proves corruption or runtime state. A source that changes during Pi's two-pass analysis returns SOURCE_CHANGED. Interrupted tails and unsupported nested Code Mode remain visible in coverage.

## Lineage and versions

Only Pi session v3 is accepted. Traceveil does not mutate or migrate v1/v2 logs. The default lineage ends at the last stored entry, including metadata entries. --leaf ID selects another recorded tip or ancestor. Abandoned sibling branches remain excluded.

Pi navigation that was never persisted cannot be reconstructed. Adapter support does not provide access to nested Code Mode calls or hidden reasoning.
