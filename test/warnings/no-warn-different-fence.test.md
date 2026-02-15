---
description = "Assign at fence 1, read at fence 2 produces no UB warning"
expect_output = "10"
expect_warnings = []
---
# Main
1. x = 10
2. **{x}**
