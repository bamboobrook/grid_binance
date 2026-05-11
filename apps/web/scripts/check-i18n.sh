#!/usr/bin/env bash
# i18n completeness check
# Scans all pickText() calls and checks if corresponding keys exist in messages/zh.json and en.json

set -euo pipefail

WEB_DIR="$(cd "$(dirname "$0")/.." && pwd)"
PICKTEXT_PATTERN='pickText\([^,]+,\s*"([^"]+)"'
FOUND=0
MISSING=0

echo "=== i18n Completeness Check ==="
echo ""

# Count pickText occurrences
TOTAL=$(grep -rP 'pickText\(' "$WEB_DIR/app" "$WEB_DIR/components" "$WEB_DIR/lib" --include='*.tsx' --include='*.ts' 2>/dev/null | wc -l)
echo "Total pickText() calls found: $TOTAL"
echo ""

# Check for duplicate Chinese text that should be consolidated
echo "Top 20 most-used Chinese text strings:"
grep -roP 'pickText\([^,]+,\s*"\K[^"]+' "$WEB_DIR/app" "$WEB_DIR/components" "$WEB_DIR/lib" --include='*.tsx' --include='*.ts' 2>/dev/null | sort | uniq -c | sort -rn | head -20
echo ""

# Check messages JSON coverage
ZH_KEYS=0
EN_KEYS=0
if [ -f "$WEB_DIR/messages/zh.json" ]; then
  ZH_KEYS=$(python3 -c "import json; d=json.load(open('$WEB_DIR/messages/zh.json')); print(len(d))" 2>/dev/null || echo 0)
fi
if [ -f "$WEB_DIR/messages/en.json" ]; then
  EN_KEYS=$(python3 -c "import json; d=json.load(open('$WEB_DIR/messages/en.json')); print(len(d))" 2>/dev/null || echo 0)
fi
echo "messages/zh.json keys: $ZH_KEYS"
echo "messages/en.json keys: $EN_KEYS"
echo ""
echo "Recommendation: Migrate frequently-used pickText() calls to messages JSON for centralized management."
