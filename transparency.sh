#!/bin/bash

# Hyprland Effects Manager
# Manage blur, opacity, and transparency effects for windows and globally

set -euo pipefail

# Configuration
readonly SCRIPT_NAME=$(basename "$0")
readonly DEFAULT_OPACITY="0.8 0.8 0.8"
readonly MIN_OPACITY="0.1 0.1 0.1"
readonly MAX_OPACITY="1.0 1.0 1.0"
readonly OPACITY_STEP=0.1

# Colors for output (only if terminal supports it)
if [[ -t 1 ]] && command -v tput >/dev/null 2>&1; then
    readonly RED=$(tput setaf 1)
    readonly GREEN=$(tput setaf 2)
    readonly YELLOW=$(tput setaf 3)
    readonly BLUE=$(tput setaf 4)
    readonly NC=$(tput sgr0)
else
    readonly RED=""
    readonly GREEN=""
    readonly YELLOW=""
    readonly BLUE=""
    readonly NC=""
fi

# Helper functions
error() {
    printf "%sError: %s%s\n" "$RED" "$1" "$NC" >&2
    exit 1
}

success() {
    printf "%s✓ %s%s\n" "$GREEN" "$1" "$NC"
}

info() {
    printf "%sℹ %s%s\n" "$BLUE" "$1" "$NC"
}

warn() {
    printf "%s⚠ %s%s\n" "$YELLOW" "$1" "$NC"
}

check_dependencies() {
    local deps=("hyprctl" "jq" "bc" "awk" "grep")
    local missing=()
    
    for dep in "${deps[@]}"; do
        if ! command -v "$dep" &> /dev/null; then
            missing+=("$dep")
        fi
    done
    
    if [[ ${#missing[@]} -gt 0 ]]; then
        error "Missing dependencies: ${missing[*]}\nInstall with: sudo apt install ${missing[*]}"
    fi
}

get_active_window() {
    local addr
    addr=$(hyprctl activewindow -j 2>/dev/null | jq -r '.address' 2>/dev/null)
    
    if [[ "$addr" == "null" || -z "$addr" ]]; then
        error "No active window found or unable to get window address"
    fi
    
    echo "$addr"
}

get_window_opacity() {
    local addr=$1
    local opacity
    
    opacity=$(hyprctl getprop address:"$addr" opacity 2>/dev/null | awk '{print $NF}' 2>/dev/null)
    
    if [[ -z "$opacity" || "$opacity" == "null" ]]; then
        warn "Unable to get opacity for window $addr, defaulting to 1.0"
        echo "1.0"
    else
        echo "$opacity"
    fi
}

get_blur_status() {
    local status
    status=$(hyprctl getoption decoration:blur:enabled 2>/dev/null | grep -oP 'int: \K\d+' 2>/dev/null)
    
    if [[ -z "$status" ]]; then
        warn "Unable to get blur status, defaulting to enabled"
        echo "1"
    else
        echo "$status"
    fi
}

toggle_all_blur() {
    local current
    current=$(get_blur_status)
    
    info "Current blur status: $([ "$current" == "1" ] && echo "enabled" || echo "disabled")"
    
    if [[ "$current" == "1" ]]; then
        hyprctl keyword decoration:blur:enabled 0
        success "Blur disabled globally"
    else
        hyprctl keyword decoration:blur:enabled 1
        success "Blur enabled globally"
    fi
}

toggle_window_opacity() {
    local addr current
    addr=$(get_active_window)
    current=$(get_window_opacity "$addr")
    
    info "Current window opacity: $current"
    
    # Consider window opaque if opacity is 1.0 or very close to it
    if (( $(echo "$current >= 0.99" | bc -l) )); then
        hyprctl dispatch setprop address:"$addr" opacity "$DEFAULT_OPACITY" 2>/dev/null
        hyprctl dispatch setprop address:"$addr" opaque off 2>/dev/null
        success "Window made transparent (opacity: $DEFAULT_OPACITY)"
    else
        hyprctl dispatch setprop address:"$addr" opacity "$MAX_OPACITY" 2>/dev/null
        hyprctl dispatch setprop address:"$addr" opaque on 2>/dev/null
        success "Window made opaque (opacity: $MAX_OPACITY)"
    fi
}

adjust_window_opacity() {
    local addr current new_opacity option
    option=${1:-""}
    
    if [[ "$option" != "i" && "$option" != "d" ]]; then
        error "Invalid option. Use 'i' to increase or 'd' to decrease opacity"
    fi
    
    addr=$(get_active_window)
    current=$(get_window_opacity "$addr")
    
    if [[ "$option" == "i" ]]; then
        # Increase opacity (less transparent)
        new_opacity=$(echo "$current + $OPACITY_STEP" | bc)
        if (( $(echo "$new_opacity > $MAX_OPACITY" | bc -l) )); then
            new_opacity=$MAX_OPACITY
        fi
        info "Increasing opacity"
    else
        # Decrease opacity (more transparent)
        new_opacity=$(echo "$current - $OPACITY_STEP" | bc)
        if (( $(echo "$new_opacity < $MIN_OPACITY" | bc -l) )); then
            new_opacity=$MIN_OPACITY
        fi
        info "Decreasing opacity"
    fi
    
    hyprctl dispatch setprop address:"$addr" opacity "$new_opacity" 2>/dev/null
    success "Window opacity adjusted: $current → $new_opacity"
}

set_window_opacity() {
    local addr opacity
    opacity=${1:-""}
    
    if [[ -z "$opacity" ]]; then
        error "Opacity value required (0.1 - 1.0)"
    fi
    
    # Validate opacity value
    if ! (( $(echo "$opacity >= $MIN_OPACITY && $opacity <= $MAX_OPACITY" | bc -l) )); then
        error "Opacity must be between $MIN_OPACITY and $MAX_OPACITY"
    fi
    
    addr=$(get_active_window)
    hyprctl dispatch setprop address:"$addr" opacity "$opacity" 2>/dev/null
    success "Window opacity set to: $opacity"
}

show_status() {
    local addr current_opacity blur_status
    
    printf "\n%s=== Hyprland Effects Status ===%s\n" "$BLUE" "$NC"
    
    # Blur status
    blur_status=$(get_blur_status)
    printf "Global Blur: "
    if [[ "$blur_status" == "1" ]]; then
        printf "%sEnabled%s\n" "$GREEN" "$NC"
    else
        printf "%sDisabled%s\n" "$RED" "$NC"
    fi
    
    # Active window info
    if addr=$(hyprctl activewindow -j 2>/dev/null | jq -r '.address' 2>/dev/null) && [[ "$addr" != "null" && -n "$addr" ]]; then
        current_opacity=$(get_window_opacity "$addr")
        local window_class=$(hyprctl activewindow -j 2>/dev/null | jq -r '.class' 2>/dev/null)
        local window_title=$(hyprctl activewindow -j 2>/dev/null | jq -r '.title' 2>/dev/null)
        
        printf "\n%sActive Window:%s\n" "$BLUE" "$NC"
        printf "  Class: %s\n" "$window_class"
        printf "  Title: %s\n" "$window_title"
        printf "  Address: %s\n" "$addr"
        printf "  Opacity: %s " "$current_opacity"
        if [[ "$(echo "$current_opacity >= 0.99" | bc -l)" == "1" ]]; then
            printf "(%sOpaque%s)\n" "$GREEN" "$NC"
        else
            printf "(%sTransparent%s)\n" "$YELLOW" "$NC"
        fi
    else
        printf "\n%sNo active window found%s\n" "$YELLOW" "$NC"
    fi
    printf "\n"
}

show_help() {
    printf "%sHyprland Effects Manager%s\n\n" "$BLUE" "$NC"
    
    printf "%sUSAGE:%s\n" "$YELLOW" "$NC"
    printf "    %s [COMMAND] [OPTIONS]\n\n" "$SCRIPT_NAME"
    
    printf "%sCOMMANDS:%s\n" "$YELLOW" "$NC"
    printf "    %sblur-toggle%s           Toggle blur on/off globally\n" "$GREEN" "$NC"
    printf "    %sopacity-toggle%s        Toggle active window between opaque and transparent\n" "$GREEN" "$NC"
    printf "    %sopacity-increase%s      Increase active window opacity by %s\n" "$GREEN" "$NC" "$OPACITY_STEP"
    printf "    %sopacity-decrease%s      Decrease active window opacity by %s\n" "$GREEN" "$NC" "$OPACITY_STEP"
    printf "    %sopacity-set%s <value>   Set active window opacity to specific value (0.1-1.0)\n" "$GREEN" "$NC"
    printf "    %sstatus%s                Show current effects status\n" "$GREEN" "$NC"
    printf "    %shelp%s                  Show this help message\n\n" "$GREEN" "$NC"
    
    printf "%sALIASES:%s\n" "$YELLOW" "$NC"
    printf "    bt, blur                → blur-toggle\n"
    printf "    ot, toggle              → opacity-toggle\n"
    printf "    oi, inc, +              → opacity-increase\n"
    printf "    od, dec, -              → opacity-decrease\n"
    printf "    os, set                 → opacity-set\n"
    printf "    st, status, info        → status\n\n"
    
    printf "%sEXAMPLES:%s\n" "$YELLOW" "$NC"
    printf "    %s blur-toggle              # Toggle blur on/off\n" "$SCRIPT_NAME"
    printf "    %s opacity-toggle           # Toggle window transparency\n" "$SCRIPT_NAME"
    printf "    %s opacity-increase         # Make window less transparent\n" "$SCRIPT_NAME"
    printf "    %s opacity-decrease         # Make window more transparent\n" "$SCRIPT_NAME"
    printf "    %s opacity-set 0.7          # Set window to 70%% opacity\n" "$SCRIPT_NAME"
    printf "    %s status                   # Show current status\n\n" "$SCRIPT_NAME"
}

main() {
    local command=${1:-"help"}
    
    # Check dependencies first (except for help)
    if [[ "$command" != "help" && "$command" != "-h" && "$command" != "--help" ]]; then
        check_dependencies
    fi
    
    case "$command" in
        "blur-toggle"|"bt"|"blur")
            toggle_all_blur
            ;;
        "opacity-toggle"|"ot"|"toggle")
            toggle_window_opacity
            ;;
        "opacity-increase"|"oi"|"inc"|"+")
            adjust_window_opacity "i"
            ;;
        "opacity-decrease"|"od"|"dec"|"-")
            adjust_window_opacity "d"
            ;;
        "opacity-set"|"os"|"set")
            set_window_opacity "$2"
            ;;
        "status"|"st"|"info")
            show_status
            ;;
        "help"|"-h"|"--help"|*)
            show_help
            ;;
    esac
}

# Execute main function with all arguments
main "$@"
