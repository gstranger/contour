#!/usr/bin/env bash
set -euo pipefail

# Requires: GitHub CLI (gh) authenticated, and the current directory is a git repo
# with a GitHub remote. Creates a v1.0 milestone and labels from .github/labels.json.

if ! command -v gh >/dev/null 2>&1; then
  echo "Error: gh (GitHub CLI) is not installed." >&2
  exit 1
fi

REPO_FULL=$(gh repo view --json nameWithOwner --jq '.nameWithOwner')

# Create milestone if missing (via REST API)
MILESTONE_NAME="v1.0"
existing=$(gh api -X GET \
  "repos/${REPO_FULL}/milestones?state=all&per_page=100" \
  --jq ".[] | select(.title==\"${MILESTONE_NAME}\").number" || true)
if [[ -z "$existing" ]]; then
  gh api -X POST "repos/${REPO_FULL}/milestones" \
    -f "title=${MILESTONE_NAME}" \
    -f "description=Initial production-ready release" \
    -f "state=open" >/dev/null
  echo "Created milestone '${MILESTONE_NAME}' in ${REPO_FULL}."
else
  echo "Milestone '${MILESTONE_NAME}' already exists in ${REPO_FULL}; skipping."
fi

# Create labels
LABEL_FILE=".github/labels.json"
if [[ ! -f "$LABEL_FILE" ]]; then
  echo "Missing $LABEL_FILE" >&2
  exit 1
fi

tmpfile=$(mktemp)
trap 'rm -f "$tmpfile"' EXIT

cat "$LABEL_FILE" | jq -c '.[]' > "$tmpfile"
while IFS= read -r line; do
  name=$(echo "$line" | jq -r '.name')
  color=$(echo "$line" | jq -r '.color')
  desc=$(echo "$line" | jq -r '.description')
  if gh label list | awk -F"\t" '{print $1}' | grep -qx "$name"; then
    gh label edit "$name" --color "$color" --description "$desc" >/dev/null
    echo "Updated label: $name"
  else
    gh label create "$name" --color "$color" --description "$desc" >/dev/null || true
    echo "Created label: $name"
  fi
done < "$tmpfile"

echo "Bootstrap complete: milestone and labels ready."
