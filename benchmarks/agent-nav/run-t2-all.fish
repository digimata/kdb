#!/usr/bin/env fish

set SCRIPT_DIR (dirname (status filename))
set TASK_DIR "$SCRIPT_DIR/t2-explain-arch"

for repo in mio airstore poetry tokio kubernetes
    fish $SCRIPT_DIR/run-one.fish $repo t2 kdb &
    fish $SCRIPT_DIR/run-one.fish $repo t2 baseline &
end

wait

echo ""
echo "All T2 runs complete. Generating results.md..."

# generate combined results
python3 "$SCRIPT_DIR/gen-results.py" "$TASK_DIR"
