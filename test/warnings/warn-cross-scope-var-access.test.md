---
description = "Cross-scope block cannot access unexecuted parent var"
expect_error = "undefined variable"
---
# Main
1. [](#Foreign)

# Owner
1. secret = 42

## Foreign
1. **{secret}**
