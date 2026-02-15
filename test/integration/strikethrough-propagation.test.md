---
description = "Null value threaded through blocks"
expect_output = "was null"
---
# Main
1. x = false ? 42
2. **{[x](#Check)}**

## Check
1. match #0
    - ~~doc~~: "was null"
    - otherwise: "had value"
