---
description = "Two assignments at same fence do not crash, but produce warning"

[[expect_warnings]]
contains = "same fence"
---
# Main
1. x = 1
1. x = 2
2. **{x}**
