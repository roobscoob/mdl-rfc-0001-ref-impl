---
description = "Cross-scope block accessing parent var warns"
expect_warnings = [{ contains = "non-lexical" }]
---
# Main
1. [](#Foreign)

# Owner
1. secret = 42

## Foreign
1. **{secret}**
