---
description = "Sub-block reads parent var assigned at later fence"

[[expect_warnings]]
contains = "before assignment"
---
# Main
1. [](#Child)
2. x = 42

## Child
1. **{x}**
