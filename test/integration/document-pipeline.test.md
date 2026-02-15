---
description = "Pass documents between blocks"
expect_output = "Hello, world!"
---
# Main
1. doc = [](#GetDoc)
2. **{doc}**

## GetDoc
1. [](#Content)

## Content
Hello, world!
