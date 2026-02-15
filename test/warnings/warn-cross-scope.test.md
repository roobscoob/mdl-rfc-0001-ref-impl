---
description = "Warning for cross-scope sub-block invocation"
expect_warnings = [{ contains = "non-lexical scope" }]
---
# Main
1. x = 5
2. [](#OtherChild)

# Other
## OtherChild
1. **{x}**
