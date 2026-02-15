---
description = "Multiple UB warnings in one program"
expect_warnings = [
    { contains = "before assignment" },
    { contains = "before assignment" }
]
---
# Main
1. **{x}**
1. **{y}**
2. x = 1
3. y = 2
