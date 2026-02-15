---
description = "Match arm result is complex expression"
expect_output = "15"
---
# Main
1. x = match 5
    - 5: 5 + 10
    - otherwise: 0
2. **{x}**
