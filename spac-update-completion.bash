#!/usr/bin/env bash

_spac-update() {
    local cur opts
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    opts="from-bower prepare-bower update-all help"

    if [ "$COMP_CWORD" == 1 ]
    then
        # shellcheck disable=SC2207
        # shellcheck disable=SC2086
        COMPREPLY=( $(compgen -W "${opts}" -- ${cur}) );
        return 0;
    fi
}

complete -F _spac-update spac-update
