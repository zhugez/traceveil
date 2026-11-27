# Your first report

Install the candidate using [installation](installation.md). Then run:

~~~sh
traceveil --version
traceveil demo
traceveil inspect f001 --demo
traceveil doctor
~~~

The demo contains three labeled synthetic patterns. Inspection shows normalized tool categories, statuses and source record lines. Raw source text stays private.

## Analyze this Pi project

Run from the project whose sessions you want to inspect:

~~~sh
traceveil analyze --last
~~~

The command reads one resolved session directory. It selects the newest modified, supported Pi v3 file with matching or absent project metadata. It does not prove that the session is finished. Header paths never grant access to another directory.

For a custom authorized location:

~~~sh
traceveil init --session-dir DIR
traceveil analyze --last
~~~

Replace DIR with an existing session directory. This saves your user configuration. It does not modify Pi settings or install hooks. Use [Pi discovery](pi.md) if no session appears.

## Inspect your finding

Copy the full reference printed by analysis:

~~~text
traceveil inspect REFERENCE SESSION --agent pi
~~~

Replace SESSION with the exact source. Repeat the same leaf and rule options. For discovery, you can repeat the original --last, --session-dir and --project options. A file that changes between analysis and inspection produces STALE_REFERENCE. Select a stable, authorized copy if you need repeatable inspection.

Check missing evidence before acting. Repetition can be intentional. A recommendation is a possible investigation, not an instruction to execute commands from a transcript.
