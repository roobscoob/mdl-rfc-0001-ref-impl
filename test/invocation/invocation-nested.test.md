---
description = "Nested invocations: A calls B calls C"
expect_output = "deep"
---
# Main
1. **{[](#A)}**

## A
1. [](#B)

## B
1. [](#C)

## C
1. "deep"
