---
description = "Sub-block writing to parent variable is visible after return"
expect_output = "modified"
---
# Main
1. x = "original"
2. [](#Child)
3. **{x}**

## Child
1. x = "modified"
