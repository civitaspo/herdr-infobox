#!/bin/sh
set -eu

fixture_case=${0##*/}
case "$fixture_case" in
    invalid) exit 99 ;;
    unknown-cli) printf 'opencode v2.0.19\n'; exit ;;
esac
version=2.0.18
if [ "$fixture_case" = success-2.0.15 ]; then version=2.0.15; fi
if [ "$1" = --version ]; then
    printf 'opencode v%s\n' "$version"
    exit
fi
if [ "$5" = /api/info ]; then
    if [ "$fixture_case" = unknown-server ]; then
        printf '{"version":"2.0.19"}\n'
    else
        printf '{"version":"%s"}\n' "$version"
    fi
    exit
fi
case "$fixture_case" in
    success|success-2.0.15)
        printf '%s\n' "$@" > "$0.args"
        printf '{"data":{"info":{"id":"ses_fixture"},"messages":[]}}\n'
        ;;
    unknown-server) touch "$0.exported" ;;
    failed) printf 'private-session-body'; printf 'secret-token' >&2; exit 1 ;;
    oversized) exec dd if=/dev/zero bs=1048576 count=17 2>/dev/null ;;
    hanging) sleep 30 ;;
    *) exit 99 ;;
esac
