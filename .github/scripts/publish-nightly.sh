#!/usr/bin/env bash
set -euo pipefail

tag=nightly-builds
repo="repos/$GITHUB_REPOSITORY"
main_sha=$(gh api "$repo/commits/main" --jq .sha)
if [[ $main_sha != "$GITHUB_SHA" ]]; then
  echo "A newer commit is on main. Skipping publication of $GITHUB_SHA."
  exit 0
fi

mkdir -p nightly
cp packages/FoxTODO-windows-x64/*.exe nightly/FoxTODO-windows-x64-nightly.exe
cp packages/FoxTODO-macos-arm64/*.dmg nightly/FoxTODO-macos-arm64-nightly.dmg
cp packages/FoxTODO-android-arm64-signed/FoxTODO-android-arm64-release.apk nightly/FoxTODO-android-arm64-nightly.apk

title="FoxTODO Nightly"
notes="来自 [main 的提交 ${GITHUB_SHA:0:7}]($GITHUB_SERVER_URL/$GITHUB_REPOSITORY/commit/$GITHUB_SHA) · [构建记录]($GITHUB_SERVER_URL/$GITHUB_REPOSITORY/actions/runs/$GITHUB_RUN_ID)"
if ! gh release view "$tag" --json id >/dev/null; then
  gh release create "$tag" nightly/* --target "$GITHUB_SHA" --title "$title" \
    --notes "$notes" --prerelease --latest=false
  exit 0
fi

# Stage all uploads before replacing any of the public download links.
files=(nightly/*)
for file in "${files[@]}"; do mv "$file" "$file.pending"; done
gh release upload "$tag" nightly/*.pending --clobber

release_id=$(gh release view "$tag" --json databaseId --jq .databaseId)
assets=$(gh api --paginate --slurp "$repo/releases/$release_id/assets" | jq 'add')
for file in "${files[@]}"; do
  name=${file##*/}
  pending=$(jq -er --arg name "$name.pending" '.[] | select(.name == $name and .state == "uploaded") | .id' <<< "$assets")
  old=$(jq -r --arg name "$name" '.[] | select(.name == $name) | .id' <<< "$assets")
  if [[ -n $old ]]; then gh api --silent -X DELETE "$repo/releases/assets/$old"; fi
  gh api --silent -X PATCH "$repo/releases/assets/$pending" -f name="$name"
done

# A draft left by an interrupted first publication may not have a tag yet.
has_tag=$(gh api "$repo/git/matching-refs/tags/$tag" --jq 'any(.[]; .ref == "refs/tags/nightly-builds")')
if [[ $has_tag == true ]]; then
  gh api --silent -X PATCH "$repo/git/refs/tags/$tag" -f sha="$GITHUB_SHA" -F force=true
fi
gh release edit "$tag" --target "$GITHUB_SHA" --title "$title" --notes "$notes" \
  --draft=false --prerelease --latest=false
