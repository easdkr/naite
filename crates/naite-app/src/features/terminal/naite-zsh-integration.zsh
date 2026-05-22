# naite zsh integration. Sourced from the wrapper .zshrc.
# Emits OSC 777;naite;<event>;<field>... <BEL> events.

# Bail out early if not zsh.
[[ -n "$ZSH_VERSION" ]] || return 0

# Percent-encode a single string. Encodes ;, %, control chars, and bytes >= 0x80.
# Result printed to stdout.
_naite_pct_encode() {
    emulate -L zsh
    local s="$1"
    local out=""
    local i char hex
    for ((i = 1; i <= ${#s}; i++)); do
        char="${s[i]}"
        case "$char" in
            ';'|'%'|$'\x07'|$'\x1b'|$'\n'|$'\r'|$'\t')
                printf -v hex '%%%02X' "'$char"
                out+="$hex"
                ;;
            *)
                out+="$char"
                ;;
        esac
    done
    print -nr -- "$out"
}

# Emit one OSC event. Usage: _naite_emit <event_name> [field...]
_naite_emit() {
    emulate -L zsh
    local event="$1"
    shift
    local out
    out=$'\e]777;naite;'"$event"
    local field
    for field in "$@"; do
        out+=';'"$(_naite_pct_encode "$field")"
    done
    out+=$'\a'
    printf '%s' "$out"
}

# Emit recent history. Best-effort; relies on fc availability.
_naite_emit_history() {
    emulate -L zsh
    local -a hist
    hist=("${(@f)$(fc -ln -200 2>/dev/null)}")
    _naite_emit history "${hist[@]}"
}

# State: track whether ready has been sent.
typeset -g _naite_ready_sent=0

_naite_precmd() {
    emulate -L zsh
    local last_exit=$?
    if (( _naite_ready_sent == 0 )); then
        _naite_emit ready
        _naite_emit_history
        _naite_ready_sent=1
    fi
    if [[ -n "$_naite_command_running" ]]; then
        _naite_emit command_finish "$last_exit"
        unset _naite_command_running
    fi
    _naite_emit cwd "$PWD"
}

_naite_preexec() {
    emulate -L zsh
    _naite_command_running=1
    _naite_emit command_start "$1"
}

# zle widget to track input buffer changes.
_naite_input_changed() {
    emulate -L zsh
    _naite_emit input "$BUFFER" "$CURSOR"
}

# Wrapper that runs the user's widget if any, then emits input state.
# We hook into zle-line-init, zle-line-finish, and use a periodic check via
# zle-line-pre-redraw which fires after every keystroke.
_naite_zle_line_init() {
    _naite_emit input "$BUFFER" "$CURSOR"
    zle && zle .accept-line-line-init 2>/dev/null
}

_naite_zle_line_pre_redraw() {
    _naite_emit input "$BUFFER" "$CURSOR"
}

# Install hooks. Use add-zsh-hook when available (zsh >= 5.0).
autoload -Uz add-zsh-hook 2>/dev/null
if (( $+functions[add-zsh-hook] )); then
    add-zsh-hook precmd _naite_precmd
    add-zsh-hook preexec _naite_preexec
fi

# Install zle widget for input tracking.
# zle-line-pre-redraw requires zsh 5.3+; if unavailable input events won't fire
# but precmd/preexec/cwd still work (graceful degradation).
if [[ -n "$ZLE_VERSION" ]] || (( $+widgets )); then
    zle -N _naite_input_changed
    if [[ -n "$widgets[zle-line-pre-redraw]" ]]; then
        # User already has a hook; chain ours
        functions[_naite_user_pre_redraw]=$functions[zle-line-pre-redraw]
        zle-line-pre-redraw() {
            _naite_user_pre_redraw "$@"
            _naite_zle_line_pre_redraw
        }
        zle -N zle-line-pre-redraw
    else
        zle -N zle-line-pre-redraw _naite_zle_line_pre_redraw
    fi
fi
