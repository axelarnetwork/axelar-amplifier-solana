#!/usr/bin/env bash
set -euo pipefail

echo "Generating Codama clients for all v2 programs..."

# Generate clients for each v2 program
echo " Gateway v2"
npx codama run js --config ./programs/axelar-solana-gateway-v2/codama.js

echo " Gas Service v2"
npx codama run js --config ./programs/axelar-solana-gas-service-v2/codama.js

echo " Governance v2"
npx codama run js --config ./programs/axelar-solana-governance-v2/codama.js

echo " Memo v2"
npx codama run js --config ./programs/axelar-solana-memo-v2/codama.js

# Format generated files if prettier is available
if command -v prettier &>/dev/null; then
    echo "Formatting generated files with prettier..."
    npx prettier --write './programs/*/clients/js/src/generated/{*,**/*}.{ts,js}' 2>/dev/null || true
fi

echo "Client generation complete!"
