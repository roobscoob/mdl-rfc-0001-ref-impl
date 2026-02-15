---
description = "Sub-block shadows parent variable, parent unchanged after return"
expect_output = "original"
---
# Main
1. x = "original"
2. [](#Child)
3. **{x}**

## Child
1. x = "modified"
