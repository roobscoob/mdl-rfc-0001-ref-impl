---
description = "Arguments to sub-block do not leak to parent"
expect_output = "5"
---
# Main
1. x = 5
2. [99](#Child)
3. **{x}**

## Child
1. #0
