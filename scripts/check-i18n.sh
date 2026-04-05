#!/bin/bash
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
I18N_DIR="$REPO_ROOT/i18n"
SOURCE="$I18N_DIR/en.json"

if [ ! -f "$SOURCE" ]; then
    echo "ERROR: Source file $SOURCE not found"
    exit 1
fi

SOURCE_KEYS=$(python3 -c "
import json
with open('$SOURCE') as f:
    data = json.load(f)
for key in sorted(data.keys()):
    print(key)
")

MISSING=0
for locale_file in "$I18N_DIR"/*.json; do
    locale=$(basename "$locale_file" .json)
    [ "$locale" = "en" ] && continue
    LOCALE_KEYS=$(python3 -c "
import json
with open('$locale_file') as f:
    data = json.load(f)
for key in sorted(data.keys()):
    print(key)
")
    DIFF=$(comm -23 <(echo "$SOURCE_KEYS") <(echo "$LOCALE_KEYS"))
    if [ -n "$DIFF" ]; then
        echo "MISSING in $locale.json:"
        echo "$DIFF" | sed 's/^/  - /'
        MISSING=1
    fi
done

if [ "$MISSING" -eq 0 ]; then
    echo "All locales have all keys from en.json"
fi
exit $MISSING
