autoload -Uz add-zsh-hook
zmodload zsh/datetime 2>/dev/null

if [[ -z "${EMYS_SESSION:-}" || "${EMYS_SHLVL:-}" != "$SHLVL" ]]; then
  typeset -gx EMYS_SESSION="${HOST:-unknown}:$$:${EPOCHREALTIME:-0}"
  typeset -gx EMYS_SHLVL="$SHLVL"
fi

typeset -g __emys_command=""
typeset -g __emys_directory=""
typeset -g __emys_started_at=""

_emys_preexec() {
  emulate -L zsh

  __emys_command="$1"
  __emys_directory="$PWD"
  __emys_started_at="${EPOCHREALTIME:-}"
}

_emys_precmd() {
  local exit_code="$?"
  emulate -L zsh

  local finished_at="${EPOCHREALTIME:-}"

  if [[ -z "$__emys_command" ]]; then
    return "$exit_code"
  fi

  local command="$__emys_command"
  local directory="$__emys_directory"
  local started_at="$__emys_started_at"
  local timestamp_ns=""
  local duration_ns=""
  local -a arguments=(
    --directory "$directory"
    --exit-code "$exit_code"
    --session "$EMYS_SESSION"
    --shell zsh
  )

  __emys_command=""
  __emys_directory=""
  __emys_started_at=""

  if [[ -n "$started_at" && -n "$finished_at" ]]; then
    printf -v timestamp_ns '%.0f' "$((started_at * 1000000000))"
    printf -v duration_ns '%.0f' "$(((finished_at - started_at) * 1000000000))"
    arguments+=(--timestamp-ns "$timestamp_ns" --duration-ns "$duration_ns")
  fi

  if [[ -n "${HOST:-}" ]]; then
    arguments+=(--hostname "$HOST")
  fi

  command emys add "${arguments[@]}" -- "$command" >/dev/null 2>&1

  return "$exit_code"
}

_emys_search() {
  emulate -L zsh

  zle -I

  local selected
  selected="$(command emys search --interactive -- "$BUFFER")"
  local exit_code="$?"

  if (( exit_code == 0 )) && [[ -n "$selected" ]]; then
    BUFFER="$selected"
    CURSOR="${#BUFFER}"
  fi

  zle reset-prompt

  return "$exit_code"
}

add-zsh-hook preexec _emys_preexec
add-zsh-hook precmd _emys_precmd
zle -N emys-search _emys_search
bindkey '^R' emys-search
