#!/usr/bin/env bash
set -euo pipefail

PLUGIN_NAME="clean-code-skills"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Install clean-code-skills plugin for Claude Code.

Options:
  --global                Install globally to ~/.claude/plugins/
  --project <path>        Install to a specific project directory
  --skill <name>          Install only a single skill (use with --global or --project)
  --list                  List all available skills with descriptions
  --uninstall             Remove installed plugin (use with --global or --project)
  -h, --help              Show this help message

Examples:
  $(basename "$0") --global
  $(basename "$0") --project /path/to/project
  $(basename "$0") --skill tdd-red-green-refactor --global
  $(basename "$0") --uninstall --global
  $(basename "$0") --uninstall --project /path/to/project
EOF
}

list_skills() {
  echo "Available skills:"
  echo ""
  for skill_dir in "$SCRIPT_DIR"/skills/*/; do
    skill_name="$(basename "$skill_dir")"
    description=""
    if [[ -f "$skill_dir/SKILL.md" ]]; then
      description=$(grep '^description:' "$skill_dir/SKILL.md" | head -1 | sed 's/^description: //')
    fi
    printf "  %-50s %s\n" "$skill_name" "$description"
  done
}

install_plugin() {
  local dest="$1"
  local skill_filter="${2:-}"

  mkdir -p "$dest"

  if [[ -n "$skill_filter" ]]; then
    if [[ ! -d "$SCRIPT_DIR/skills/$skill_filter" ]]; then
      echo "Error: Skill '$skill_filter' not found."
      echo "Run '$(basename "$0") --list' to see available skills."
      exit 1
    fi
    mkdir -p "$dest/skills/$skill_filter"
    cp -r "$SCRIPT_DIR/skills/$skill_filter/"* "$dest/skills/$skill_filter/"
    echo "Installed skill '$skill_filter' to $dest"
  else
    cp -r "$SCRIPT_DIR/.claude-plugin" "$dest/"
    cp -r "$SCRIPT_DIR/skills" "$dest/"
    cp -r "$SCRIPT_DIR/commands" "$dest/"
    echo "Installed $PLUGIN_NAME to $dest"
  fi
}

uninstall_plugin() {
  local dest="$1"
  if [[ -d "$dest" ]]; then
    rm -rf "$dest"
    echo "Uninstalled $PLUGIN_NAME from $dest"
  else
    echo "Nothing to uninstall at $dest"
  fi
}

MODE=""
PROJECT_PATH=""
SKILL_NAME=""
UNINSTALL=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --global)
      MODE="global"
      shift
      ;;
    --project)
      MODE="project"
      PROJECT_PATH="$2"
      shift 2
      ;;
    --skill)
      SKILL_NAME="$2"
      shift 2
      ;;
    --list)
      list_skills
      exit 0
      ;;
    --uninstall)
      UNINSTALL=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1"
      usage
      exit 1
      ;;
  esac
done

if [[ -z "$MODE" ]]; then
  echo "Error: Specify --global or --project <path>."
  echo ""
  usage
  exit 1
fi

if [[ "$MODE" == "global" ]]; then
  DEST="$HOME/.claude/plugins/$PLUGIN_NAME"
elif [[ "$MODE" == "project" ]]; then
  if [[ -z "$PROJECT_PATH" ]]; then
    echo "Error: --project requires a path argument."
    exit 1
  fi
  DEST="$PROJECT_PATH/.claude/plugins/$PLUGIN_NAME"
fi

if [[ "$UNINSTALL" == true ]]; then
  uninstall_plugin "$DEST"
else
  install_plugin "$DEST" "$SKILL_NAME"
fi
