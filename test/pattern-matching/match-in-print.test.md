---
description = "Match expression directly in print"
expect_output = "two"
---
# Main
1. **{match 2
    - 1: "one"
    - 2: "two"
    - otherwise: "other"}**
