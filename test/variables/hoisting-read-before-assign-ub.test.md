---
description = "Reading variable before assignment is UB (warning)"

[[expect_warnings]]
contains = "before assignment"
---
# Main
1. **{x}**
2. x = 42
