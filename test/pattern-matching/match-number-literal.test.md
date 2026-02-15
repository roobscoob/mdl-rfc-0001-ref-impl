---
description = "Match on number literal"
expect_output = "two"
---
# Main
1. x = match 2
    - 1: "one"
    - 2: "two"
    - 3: "three"
    - otherwise: "other"
2. **{x}**
