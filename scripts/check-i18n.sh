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

# Check that all literal t('...') keys used in the desktop frontend exist
FRONTEND_SRC="$REPO_ROOT/crates/visio-desktop/frontend/src"
USED_KEYS=$(FRONTEND_SRC="$FRONTEND_SRC" python3 <<'PYEOF'
import os, re, pathlib
src = pathlib.Path(os.environ["FRONTEND_SRC"])
keys = set()
for f in list(src.rglob("*.ts")) + list(src.rglob("*.tsx")):
    keys.update(re.findall(r"(?<![A-Za-z0-9_])t\('([A-Za-z0-9_.]+)'", f.read_text()))
for k in sorted(keys):
    print(k)
PYEOF
)
MISSING_USED=$(comm -23 <(echo "$USED_KEYS") <(echo "$SOURCE_KEYS"))
if [ -n "$MISSING_USED" ]; then
    echo "USED in frontend but MISSING in en.json:"
    echo "$MISSING_USED" | sed 's/^/  - /'
    MISSING=1
fi

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
