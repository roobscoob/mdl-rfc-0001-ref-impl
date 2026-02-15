---
description = "Chain of calls through nested sub-blocks"
expect_output = "30"
---
# Main
1. **{[10](#Double)}**

## Double
1. **{[#0 * 2](#AddTen)}**

### AddTen
1. #0 + 10
