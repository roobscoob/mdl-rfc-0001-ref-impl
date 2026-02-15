---
description = "Read and write at same fence index is UB (warning)"
expect_warnings = [{ contains = "same fence" }]
---
# Main
1. x = 10
1. **{x}**
