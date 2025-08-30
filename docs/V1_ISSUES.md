V1 issues: how to bootstrap and create

Prereqs
- Install GitHub CLI: `brew install gh` (or from github.com/cli/cli)
- Authenticate: `gh auth login`
- Ensure this repo has a GitHub remote (e.g., `origin`)
- Install `jq` for JSON parsing: `brew install jq`

1) Create milestone and labels
- Run: `bash scripts/gh_bootstrap.sh`
- This creates milestone `v1.0` and the labels defined in `.github/labels.json`.

2) Create all v1 issues
- Run: `bash scripts/create_v1_issues.sh`
- The script opens 30+ issues with titles, bodies, labels, and assigns them to the `v1.0` milestone.

Notes
- Re-running the label bootstrap is idempotent; it updates colors/descriptions.
- You can safely delete or modify any created issue afterward.
- Customize priorities/labels directly in `scripts/create_v1_issues.sh` before running if you like.
