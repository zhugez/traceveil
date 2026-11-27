# Configure Traceveil

Traceveil loads user policy for analyze. The compatibility diagnose command retains its original defaults unless explicit rule flags are supplied. The standalone demo uses built-in defaults. Inspect also starts from the original rule defaults. Repeat the pinned policy options from the report template; saved rule policy is not loaded during inspection.

~~~sh
traceveil config path
traceveil config show
traceveil config set threshold.RF001 4
traceveil config set suppress.READ001 true
traceveil config set suppress.READ001 false
~~~

Use RF001, LOOP001 or READ001. Thresholds accept 3 through 16. Configuration rejects unknown keys. Traceveil does not automatically load repository-controlled Traceveil policy.

## Location and precedence

Configuration uses TRACEVEIL_CONFIG when set. Otherwise it uses XDG_CONFIG_HOME/traceveil/config.json, or HOME/.config/traceveil/config.json. USERPROFILE is the home fallback. No configuration file is created during analysis.

For rule thresholds, the most specific value wins: --rule-threshold RULE=COUNT, then a user rule threshold, then --repeat-threshold, whose default is 3. --suppress adds explicit suppressions. Set suppress.RULE to false to remove a saved suppression.

~~~sh
traceveil analyze --demo --rule-threshold RF001=4 --suppress READ001
~~~

The time gap defaults to 600 seconds. --max-gap-seconds accepts 1 through 86400. Policy affects report references. Inspect with the same effective policy. LOOP001 thresholds count repetitions of a two-to-four-action sequence; its occurrences field counts participating operations.

## Authorize a custom session directory

~~~sh
traceveil init --session-dir DIR
traceveil analyze --last
~~~

Saved session_dir must be absolute. init and config set session-dir canonicalize an existing directory. These commands authorize that location for future discovery. They do not modify Pi's configuration.

Config input is limited to 64 KiB. Writes use a private temporary file and atomic replacement. Unix files use mode 0600 and newly created directories use 0700. Existing directory permissions stay unchanged. Symlinked configuration or skill destinations are refused. Use a physical path when a home or configuration directory is a symlink.

Windows code includes reparse-point checks and replacement preparation. Native Windows filesystem and terminal behavior is not validated by this Linux run. See [release gates](releasing.md).
