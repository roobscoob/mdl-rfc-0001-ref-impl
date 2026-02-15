---
description = "Cross-scope invocation produces warning"

[[expect_warnings]]
contains = "non-lexical scope"
---
# Main
1. [](#OtherChild)

# Other
## OtherChild
1. **{"crossed"}**
