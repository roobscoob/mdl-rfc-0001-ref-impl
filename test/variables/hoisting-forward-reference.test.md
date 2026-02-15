---
description = "Hoisted variable forward reference produces UB warning not error"

[[expect_warnings]]
contains = "before assignment"
---
# Main
1. **{y}**
2. y = 10
