#!/bin/sh

set -eu

repository_dir=$(CDPATH= cd -P -- "$(dirname -- "$0")/.." && pwd)
temporary_dir=$(mktemp -d)
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM

ln -s "$repository_dir/scripts/launch.sh" "$temporary_dir/peek-codex-launch"
version=$(cd /tmp && "$temporary_dir/peek-codex-launch" --version)

test "$version" = "peek-codex 0.1.0"
