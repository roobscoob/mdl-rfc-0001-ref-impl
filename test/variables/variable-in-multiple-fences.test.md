---
description = "Assign at fence 1, read at fence 2 is safe"
expect_output = "42"
expect_warnings = []
---
# Main
1. x = 42
2. **{x}**
