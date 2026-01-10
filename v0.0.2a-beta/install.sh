#!/bin/bash

# ============================================
# CONFIGURATION - EDIT THESE THREE VARIABLES
# ============================================

# 1. Installation destination
DEST="/usr/local/bin"

# 2. Operation: "mv" to move, "cp" to copy
OPERATION="cp"

# 3. Target name & prefix to detect (e.g., vmate-cli will find vmate-cli-*)
TARGET_PREFIX="vmate-cli"



set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Check root for system directories
if [[ "$DEST" =~ ^/(usr|opt|etc)/ ]] && [ "$EUID" -ne 0 ]; then
    echo -e "${RED}Error:${NC} Need root access for $DEST"
    echo "Run: sudo $0"
    exit 1
fi

# Functions
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

# Direct uninstall flag
if [ "$1" = "--uninstall" ]; then
    uninstall
fi

# Main installation
DEST_FILE="$DEST/$TARGET_PREFIX"
mkdir -p "$DEST"

# Detect existing installation
if [ -f "$DEST_FILE" ]; then
    echo -e "${BLUE}Existing installation detected:${NC} $DEST_FILE"
    echo -e "Choose: [${YELLOW}u${NC}]ninstall | [${YELLOW}r${NC}]eplace | [${YELLOW}c${NC}]ancel"
    read -p "Action: " choice
    
    case "$choice" in
        u|U)
            uninstall
            ;;
        r|R)
            rm -f "$DEST_FILE"
            echo -e "${YELLOW}Replacing existing version...${NC}"
            ;;
        *)
            echo "Cancelled."
            exit 0
            ;;
    esac
fi

# Find source file (auto-detects vmate-cli-*)
shopt -s nullglob
CANDIDATES=("$TARGET_PREFIX"*)
shopt -u nullglob

if [ ${#CANDIDATES[@]} -eq 0 ]; then
    echo -e "${RED}Error:${NC} No source file found matching '$TARGET_PREFIX*'"
    exit 1
fi

SOURCE_FILE="${CANDIDATES[0]}"
if [ ${#CANDIDATES[@]} -gt 1 ]; then
    echo -e "${YELLOW}Warning:${NC} Multiple files found, using: $SOURCE_FILE"
fi

# Install
echo -e "${GREEN}Installing:${NC} $SOURCE_FILE → $DEST_FILE"
if [ "$OPERATION" = "mv" ]; then
    mv "$SOURCE_FILE" "$DEST_FILE"
elif [ "$OPERATION" = "cp" ]; then
    cp "$SOURCE_FILE" "$DEST_FILE"
else
    echo -e "${RED}Error:${NC} OPERATION must be 'mv' or 'cp'"
    exit 1
fi

chmod +x "$DEST_FILE"
echo -e "${GREEN}✓ Installed${NC} $TARGET_PREFIX to $DEST"
echo -e "${BLUE}Usage:${NC} sudo $0 --uninstall  (to remove later)"