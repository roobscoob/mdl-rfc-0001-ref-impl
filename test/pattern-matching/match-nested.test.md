---
description = "Nested match expression"
expect_output = "inner two"
---
# Main
1. x = match 1
    - 1: match 2
        - 1: "inner one"
        - 2: "inner two"
        - otherwise: "inner other"
    - otherwise: "outer other"
2. **{x}**
