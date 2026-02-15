---
description = "Warning when reading and writing at same fence"
expect_warnings = [{ contains = "same fence" }]
---
# Main
1. x = 10
1. **{x}**
