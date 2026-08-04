autoload -Uz add-zsh-hook
zmodload zsh/datetime 2>/dev/null

if [[ -z "${HONU_SESSION:-}" || "${HONU_SHLVL:-}" != "$SHLVL" ]]; then
  typeset -gx HONU_SESSION="${HOST:-unknown}:$$:${EPOCHREALTIME:-0}"
  typeset -gx HONU_SHLVL="$SHLVL"
fi

typeset -g __honu_command=""
typeset -g __honu_directory=""
typeset -g __honu_started_at=""

_honu_preexec() {
  emulate -L zsh

  __honu_command="$1"
  __honu_directory="$PWD"
  __honu_started_at="${EPOCHREALTIME:-}"
}

_honu_precmd() {
  local exit_code="$?"
  emulate -L zsh

  local finished_at="${EPOCHREALTIME:-}"

  if [[ -z "$__honu_command" ]]; then
    return "$exit_code"
  fi

  local command="$__honu_command"
  local directory="$__honu_directory"
  local started_at="$__honu_started_at"
  local timestamp_ns=""
  local duration_ns=""
  local -a arguments=(
    --directory "$directory"
    --exit-code "$exit_code"
    --session "$HONU_SESSION"
    --shell zsh
  )

  __honu_command=""
  __honu_directory=""
  __honu_started_at=""

  if [[ -n "$started_at" && -n "$finished_at" ]]; then
    printf -v timestamp_ns '%.0f' "$((started_at * 1000000000))"
    printf -v duration_ns '%.0f' "$(((finished_at - started_at) * 1000000000))"
    arguments+=(--timestamp-ns "$timestamp_ns" --duration-ns "$duration_ns")
  fi

  if [[ -n "${HOST:-}" ]]; then
    arguments+=(--hostname "$HOST")
  fi

  command honu add "${arguments[@]}" -- "$command" >/dev/null 2>&1

  return "$exit_code"
}

_honu_search() {
  emulate -L zsh

  zle -I

  local selected
  selected="$(command honu search -- "$BUFFER")"
  local exit_code="$?"

  if (( exit_code == 0 )) && [[ -n "$selected" ]]; then
    BUFFER="$selected"
    CURSOR="${#BUFFER}"
  fi

  zle reset-prompt

  return "$exit_code"
}

add-zsh-hook preexec _honu_preexec
add-zsh-hook precmd _honu_precmd
zle -N honu-search _honu_search
bindkey '^R' honu-search
