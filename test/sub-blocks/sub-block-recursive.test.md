---
description = "Sub-block calls itself recursively (countdown)"
expect_output = "3\n2\n1\n0"
---
# Main
1. [3](#Count)

## Count
1. **{#0}**
2. #0 > 0 ? [#0 - 1](#Count)
