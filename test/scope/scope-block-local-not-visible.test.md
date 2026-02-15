---
description = "Variable defined in sub-block not visible in parent"
expect_error = "undefined variable"
---
# Main
1. [](#Child)
2. **{secret}**

## Child
1. secret = 42
