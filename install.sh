#!/bin/bash
# ============================================
# vmate-cli installer — place the binary on your PATH
#
# Usage:
#   ./install.sh                 install (prompts if a version already exists)
#   sudo ./install.sh            same, when DEST needs root
#   ./install.sh --uninstall     remove an existing install
#
# The binary is looked up next to this script (it ships in the same zip /
# release folder), so you can run the script from anywhere.
# ============================================

# ------- CONFIGURATION (edit if you want a user-local install) -------
DEST="/usr/local/bin"     # installation destination
OPERATION="cp"            # "mv" to move, "cp" to copy
TARGET_PREFIX="vmate-cli" # binary name

set -e

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; NC='\033[0m'

# Always resolve relative to this script, not the caller's CWD.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# System directories need root.
if [[ "$DEST" =~ ^/(usr|opt|etc)/ ]] && [ "$EUID" -ne 0 ]; then
    echo -e "${RED}Error:${NC} Need root access for $DEST"
    echo "Run: sudo $0"
    exit 1
fi

uninstall() {
    local target="$DEST/$TARGET_PREFIX"
    if [ -f "$target" ]; then
        echo -e "${YELLOW}Uninstalling:${NC} $target"
        rm -f "$target"
        echo -e "${GREEN}Success:${NC} Uninstalled $TARGET_PREFIX"
    else
        echo -e "${YELLOW}Warning:${NC} $target not found"
    fi
    exit 0
}

# Direct uninstall flag.
if [ "$1" = "--uninstall" ]; then
    uninstall
fi

# Locate the source binary (auto-detects vmate-cli / vmate-cli-*).
shopt -s nullglob
CANDIDATES=("$SCRIPT_DIR"/"$TARGET_PREFIX"*)
shopt -u nullglob

if [ ${#CANDIDATES[@]} -eq 0 ]; then
    echo -e "${RED}Error:${NC} No file matching '$TARGET_PREFIX*' next to this script."
    exit 1
fi

SOURCE_FILE="${CANDIDATES[0]}"
if [ ${#CANDIDATES[@]} -gt 1 ]; then
    echo -e "${YELLOW}Warning:${NC} Multiple files found, using: $SOURCE_FILE"
fi

DEST_FILE="$DEST/$TARGET_PREFIX"

# Detect an existing installation.
if [ -f "$DEST_FILE" ]; then
    echo -e "${BLUE}Existing installation detected:${NC} $DEST_FILE"
    echo -e "Choose: [${YELLOW}u${NC}]ninstall | [${YELLOW}r${NC}]eplace | [${YELLOW}c${NC}]ancel"
    read -p "Action: " choice

    case "$choice" in
        u|U) uninstall ;;
        r|R) rm -f "$DEST_FILE"; echo -e "${YELLOW}Replacing existing version...${NC}" ;;
        *)   echo "Cancelled."; exit 0 ;;
    esac
fi

# Install.
mkdir -p "$DEST"
echo -e "${GREEN}Installing:${NC} $SOURCE_FILE → $DEST_FILE"
case "$OPERATION" in
    mv) mv "$SOURCE_FILE" "$DEST_FILE" ;;
    cp) cp "$SOURCE_FILE" "$DEST_FILE" ;;
    *)  echo -e "${RED}Error:${NC} OPERATION must be 'mv' or 'cp'"; exit 1 ;;
esac
chmod +x "$DEST_FILE"

echo -e "${GREEN}✓ Installed${NC} $TARGET_PREFIX to $DEST"
echo "Run 'vmate-cli --help' to get started."
echo -e "${BLUE}Usage:${NC} sudo $0 --uninstall  (to remove later)"
