---
description = "Assign result of match expression to variable"
expect_output = "two"
---
# Main
1. x = match 2
    - 1: "one"
    - 2: "two"
    - otherwise: "other"
2. **{x}**
