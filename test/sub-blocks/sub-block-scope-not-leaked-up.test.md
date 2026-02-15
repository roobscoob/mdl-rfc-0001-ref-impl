---
description = "Sub-block local variable not visible in parent"
expect_error = "undefined variable"
---
# Main
1. [](#Child)
2. **{child_var}**

## Child
1. child_var = 42
