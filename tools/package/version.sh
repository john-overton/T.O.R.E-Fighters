# Shared version resolution for the packaging scripts. Source, do not execute.
#
# Order: an explicit --version argument, then the tag the release workflow is
# running for, then `git describe`, then a development placeholder. A leading
# "v" is stripped so the version is what the package formats expect.

tore_resolve_version() {
    local explicit="$1"
    local version=""
    if [ -n "$explicit" ]; then
        version="$explicit"
    elif [ -n "${GITHUB_REF_NAME:-}" ] && [ "${GITHUB_REF_TYPE:-}" = "tag" ]; then
        version="$GITHUB_REF_NAME"
    elif version=$(git describe --tags --always --dirty 2>/dev/null); then
        :
    else
        version=""
    fi
    version="${version#v}"
    if [ -z "$version" ]; then
        version="0.0.0-dev"
    fi
    printf '%s\n' "$version"
}

# Windows Installer requires a strictly numeric three-part version. A tag such
# as "0.2.0-rc1" or a `git describe` string becomes "0.2.0"; a version with no
# numeric lead becomes 0.0.0, which is correct for a local development build.
tore_msi_version() {
    printf '%s\n' "$1" | sed -n 's/^\([0-9][0-9]*\)\.\([0-9][0-9]*\)\.\([0-9][0-9]*\).*$/\1.\2.\3/p' \
        | { read -r v || v="0.0.0"; printf '%s\n' "${v:-0.0.0}"; }
}
