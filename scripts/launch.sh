#!/bin/sh

set -eu

script_path=$0
while [ -h "$script_path" ]; do
  script_dir=$(CDPATH= cd -P -- "$(dirname -- "$script_path")" && pwd)
  script_path=$(readlink "$script_path")
  case $script_path in
    /*) ;;
    *) script_path=$script_dir/$script_path ;;
  esac
done

repository_dir=$(CDPATH= cd -P -- "$(dirname -- "$script_path")/.." && pwd)
cd "$repository_dir"

exec go run -mod=readonly ./cmd/peek-codex "$@"
