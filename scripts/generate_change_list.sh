#!/usr/bin/env bash

if [[ $# -lt 1 ]]; then
    echo "Usage: $0 <project> [<directory>..]"
    exit 1
fi

latest_tag="$(git tag --list $1-v* --sort=-version:refname | head -n 1)"
ref_range="$latest_tag..HEAD"
if [[ "..HEAD" = "$ref_range" ]]; then
    ref_range="HEAD"
fi

git --no-pager log $ref_range --no-merges --oneline --no-decorate  -- ${@:2} | while read change; do
    echo "- $change"
done