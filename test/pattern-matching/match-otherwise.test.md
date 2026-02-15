---
description = "Match falls through to otherwise"
expect_output = "unknown"
---
# Main
1. x = match 99
    - 1: "one"
    - 2: "two"
    - otherwise: "unknown"
2. **{x}**
