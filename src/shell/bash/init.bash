if [[ -z "${HONU_SESSION:-}" || "${HONU_SHLVL:-}" != "$SHLVL" ]]; then
  export HONU_SESSION="${HOSTNAME:-unknown}:$$:${EPOCHREALTIME:-0}"
  export HONU_SHLVL="$SHLVL"
fi

__honu_command=""
__honu_directory=""
__honu_started_at=""
__honu_ready=""
__honu_history_number=""

_honu_preexec() {
  __honu_command="$1"
  __honu_directory="$PWD"
  __honu_started_at="${EPOCHREALTIME:-}"
}

_honu_precmd() {
  local exit_code="$?"
  local finished_at="${EPOCHREALTIME:-}"
  local command="$__honu_command"
  local directory="$__honu_directory"
  local started_at="$__honu_started_at"
  local timestamp_ns=""
  local finished_at_ns=""
  local duration_ns=""
  local -a arguments=(
    --directory "$directory"
    --exit-code "$exit_code"
    --session "$HONU_SESSION"
    --shell bash
  )

  __honu_command=""
  __honu_directory=""
  __honu_started_at=""
  __honu_ready=""

  if [[ -z "$command" ]]; then
    return "$exit_code"
  fi

  if [[ -n "$started_at" && -n "$finished_at" ]]; then
    timestamp_ns="${started_at/./}000"
    finished_at_ns="${finished_at/./}000"
    duration_ns="$((10#$finished_at_ns - 10#$timestamp_ns))"
    arguments+=(--timestamp-ns "$timestamp_ns" --duration-ns "$duration_ns")
  fi

  if [[ -n "${HOSTNAME:-}" ]]; then
    arguments+=(--hostname "$HOSTNAME")
  fi

  command honu add "${arguments[@]}" -- "$command" >/dev/null 2>&1

  return "$exit_code"
}

_honu_arm() {
  local history
  history="$(HISTTIMEFORMAT= builtin history 1)"
  history="${history#"${history%%[![:space:]]*}"}"
  __honu_history_number="${history%%[[:space:]]*}"
  __honu_ready=1
}

_honu_debug() {
  local exit_code="$1"
  local command="$BASH_COMMAND"

  if [[ -z "$__honu_ready" ||
    "$command" == _honu_precmd ||
    "$command" == _honu_arm ||
    "$command" == _honu_search ]]; then
    return "$exit_code"
  fi

  __honu_ready=""

  local history
  history="$(HISTTIMEFORMAT= builtin history 1)"
  history="${history#"${history%%[![:space:]]*}"}"
  local history_number="${history%%[[:space:]]*}"
  history="${history#*[[:space:]]}"
  history="${history#"${history%%[![:space:]]*}"}"

  if [[ -n "$history_number" && -n "$history" ]] &&
    (( 10#$history_number > 10#${__honu_history_number:-0} )); then
    command="$history"
  fi

  _honu_preexec "$command"

  return "$exit_code"
}

_honu_search() {
  local selected
  selected="$(command honu search -- "$READLINE_LINE")"
  local exit_code="$?"

  if (( exit_code == 0 )) && [[ -n "$selected" ]]; then
    READLINE_LINE="$selected"
    READLINE_POINT="${#READLINE_LINE}"
  fi

  return "$exit_code"
}

if [[ $- == *i* && -z "${__honu_initialized:-}" ]]; then
  __honu_initialized=1
  __honu_previous_debug_trap="$(trap -p DEBUG)"

  if [[ -n "$__honu_previous_debug_trap" ]]; then
    __honu_previous_debug_trap="${__honu_previous_debug_trap#trap -- \'}"
    __honu_previous_debug_trap="${__honu_previous_debug_trap%\' DEBUG}"
    eval "__honu_previous_debug_trap='$__honu_previous_debug_trap'"
    trap "$__honu_previous_debug_trap
_honu_debug \"\$?\"" DEBUG
  else
    trap '_honu_debug "$?"' DEBUG
  fi

  if [[ "$(declare -p PROMPT_COMMAND 2>/dev/null)" == "declare -a"* ]]; then
    PROMPT_COMMAND=(_honu_precmd "${PROMPT_COMMAND[@]}" _honu_arm)
  else
    PROMPT_COMMAND="_honu_precmd
${PROMPT_COMMAND-}
_honu_arm"
  fi

  bind -x '"\C-r":_honu_search'
  _honu_arm
fi
