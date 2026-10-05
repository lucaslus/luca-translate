#!/usr/bin/env bash
set -euo pipefail
[[ $# -le 1 ]] || exit 2
case "${1:---show}" in
  --toggle) exec omarchy-shell shell toggle lucas.translate '{}' ;;
  --hide) exec omarchy-shell shell hide lucas.translate ;;
  --show) action=show ;;
  --input) action=input ;;
  --selection) action=selection ;;
  --screenshot) action=screenshot ;;
  --ocr) action=ocr ;;
  --annotate) action=annotate ;;
  *) echo 'Usage: lucas-translate-native [--show|--hide|--toggle|--input|--selection|--screenshot|--ocr|--annotate]' >&2; exit 2 ;;
esac
exec omarchy-shell shell summon lucas.translate "{\"action\":\"$action\"}"
