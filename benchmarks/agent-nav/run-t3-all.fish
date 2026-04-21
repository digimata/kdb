#!/usr/bin/env fish

set SCRIPT_DIR (dirname (status filename))
set TASK_DIR "$SCRIPT_DIR/t3-add-param"

for repo in mio airstore poetry tokio kubernetes
    fish $SCRIPT_DIR/run-one.fish $repo t3 kdb &
    fish $SCRIPT_DIR/run-one.fish $repo t3 baseline &
end

wait

echo ""
echo "All T3 runs complete. Generating results.md..."

# generate combined results
python3 "$SCRIPT_DIR/gen-results.py" "$TASK_DIR"
