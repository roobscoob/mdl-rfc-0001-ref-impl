---
description = "Match expression directly in print"
expect_output = "two"
---
# Main
1. result = match 2
    - 1: "one"
    - 2: "two"
    - otherwise: "other"
2. **{result}**
