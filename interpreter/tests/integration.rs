use std::io::Write;

fn run(source: &str) -> String {
    let parser = mdl::parser::Parser::new(source.to_string(), 0);
    let program = parser.parse().expect("parse failed");
    let mut output = Vec::new();
    interpreter::execute_program(&program, &mut output)
        .map(|(_, _)| ())
        .expect("execution failed");
    String::from_utf8(output).unwrap()
}

fn run_trimmed(source: &str) -> String {
    run(source).trim().to_string()
}

#[test]
fn arithmetic() {
    assert_eq!(run_trimmed("# M\n1. **{2 + 3}**"), "5");
    assert_eq!(run_trimmed("# M\n1. **{10 - 4}**"), "6");
    assert_eq!(run_trimmed("# M\n1. **{3 * 7}**"), "21");
    assert_eq!(run_trimmed("# M\n1. **{15 / 3}**"), "5");
    assert_eq!(run_trimmed("# M\n1. **{10 % 3}**"), "1");
}

#[test]
fn operator_precedence() {
    assert_eq!(run_trimmed("# M\n1. **{2 + 3 * 4}**"), "14");
    assert_eq!(run_trimmed("# M\n1. **{(2 + 3) * 4}**"), "20");
}

#[test]
fn unary_operators() {
    assert_eq!(run_trimmed("# M\n1. **{-5 + 10}**"), "5");
    assert_eq!(run_trimmed("# M\n1. **{!false}**"), "true");
    assert_eq!(run_trimmed("# M\n1. **{!true}**"), "false");
}

#[test]
fn boolean_logic() {
    assert_eq!(run_trimmed("# M\n1. **{true && false}**"), "false");
    assert_eq!(run_trimmed("# M\n1. **{true || false}**"), "true");
    assert_eq!(run_trimmed("# M\n1. **{5 == 5}**"), "true");
    assert_eq!(run_trimmed("# M\n1. **{5 != 3}**"), "true");
    assert_eq!(run_trimmed("# M\n1. **{3 > 5}**"), "false");
    assert_eq!(run_trimmed("# M\n1. **{3 < 5}**"), "true");
}

#[test]
fn variables_and_assignment() {
    assert_eq!(
        run_trimmed("# M\n1. x = 42\n2. **{x}**"),
        "42"
    );
    assert_eq!(
        run_trimmed("# M\n1. x = 5\n2. y = 10\n3. **{x + y}**"),
        "15"
    );
}

#[test]
fn string_literals() {
    assert_eq!(
        run_trimmed("# M\n1. **{\"hello\"}**"),
        "hello"
    );
}

#[test]
fn ternary_conditional() {
    assert_eq!(
        run_trimmed("# M\n1. x = 10 > 5 ? \"yes\" : \"no\"\n2. **{x}**"),
        "yes"
    );
    assert_eq!(
        run_trimmed("# M\n1. x = 3 > 5 ? \"yes\" : \"no\"\n2. **{x}**"),
        "no"
    );
}

#[test]
fn two_operand_conditional_truthy() {
    assert_eq!(
        run_trimmed("# M\n1. true ? **{\"printed\"}**"),
        "printed"
    );
}

#[test]
fn two_operand_conditional_falsy_strikethrough() {
    let output = run_trimmed("# M\n1. x = false ? \"gone\"\n2. **{x}**");
    assert!(output.contains("~~"), "expected strikethrough but got: {}", output);
}

#[test]
fn sub_block_invocation() {
    let src = "# Main\n1. [5](#Double)\n\n## Double\n1. **{#0 * 2}**";
    assert_eq!(run_trimmed(src), "10");
}

#[test]
fn sub_block_scope_inheritance() {
    let src = "# Main\n1. x = 10\n2. [](#Child)\n\n## Child\n1. **{x}**";
    assert_eq!(run_trimmed(src), "10");
}

#[test]
fn sub_block_document_return() {
    let src = "# Main\n1. **{[](#Greet)}**\n\n## Greet\nHello, world!";
    assert_eq!(run_trimmed(src), "Hello, world!");
}

#[test]
fn fencing_same_index() {
    let src = "# M\n1. x = 1\n1. y = 2\n2. **{x + y}**";
    assert_eq!(run_trimmed(src), "3");
}

#[test]
fn arguments_positional() {
    let src = "# Main\n1. [3, 7](#Add)\n\n## Add\n1. **{#0 + #1}**";
    assert_eq!(run_trimmed(src), "10");
}

#[test]
fn match_expression() {
    let src = r#"# M
1. x = match 2
    - 1: "one"
    - 2: "two"
    - 3: "three"
    - otherwise: "other"
2. **{x}**"#;
    assert_eq!(run_trimmed(src), "two");
}

#[test]
fn match_otherwise() {
    let src = r#"# M
1. x = match 99
    - 1: "one"
    - otherwise: "unknown"
2. **{x}**"#;
    assert_eq!(run_trimmed(src), "unknown");
}

#[test]
fn match_otherwise_binding() {
    let src = r#"# M
1. x = match 42
    - 1: "one"
    - otherwise n: n
2. **{x}**"#;
    assert_eq!(run_trimmed(src), "42");
}

#[test]
fn strikethrough_value() {
    let src = "# M\n1. x = ~~hello~~\n2. **{x}**";
    let output = run_trimmed(src);
    assert!(output.contains("~~"), "expected strikethrough: {}", output);
}

#[test]
fn nested_sub_blocks() {
    let src = r#"# Outer
1. x = 1
2. [](#Middle)

## Middle
1. y = 2
2. [](#Inner)

### Inner
1. **{x + y}**"#;
    assert_eq!(run_trimmed(src), "3");
}

#[test]
fn multiple_prints() {
    let output = run("# M\n1. **{\"a\"}**\n2. **{\"b\"}**\n3. **{\"c\"}**");
    assert_eq!(output.trim(), "a\nb\nc");
}

#[test]
fn local_import_basic() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    // Write the library file
    let lib_path = dir.path().join("math.md");
    let mut lib_file = std::fs::File::create(&lib_path).unwrap();
    write!(lib_file, "# Double\n1. **{{#0 * 2}}**\n").unwrap();

    // Write the main file
    let main_source = "# Main\n1. [5](math#Double)\n";
    let parser = mdl::parser::Parser::new(main_source.to_string(), 0);
    let program = parser.parse().expect("parse failed");
    let mut output = Vec::new();
    interpreter::execute_program_with_base(&program, &mut output, dir.path().to_path_buf())
        .map(|(_, _)| ())
        .expect("execution failed");
    let result = String::from_utf8(output).unwrap();
    assert_eq!(result.trim(), "10");
}

#[test]
fn local_import_with_extension() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    // Write the library file
    let lib_path = dir.path().join("utils.md");
    let mut lib_file = std::fs::File::create(&lib_path).unwrap();
    write!(lib_file, "# Greet\nHello from import!\n").unwrap();

    // Main file imports with explicit .md extension
    let main_source = "# Main\n1. **{[](utils.md#Greet)}**\n";
    let parser = mdl::parser::Parser::new(main_source.to_string(), 0);
    let program = parser.parse().expect("parse failed");
    let mut output = Vec::new();
    interpreter::execute_program_with_base(&program, &mut output, dir.path().to_path_buf())
        .map(|(_, _)| ())
        .expect("execution failed");
    let result = String::from_utf8(output).unwrap();
    assert_eq!(result.trim(), "Hello from import!");
}

#[test]
fn local_import_caching() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    // Write a library file
    let lib_path = dir.path().join("lib.md");
    let mut lib_file = std::fs::File::create(&lib_path).unwrap();
    write!(lib_file, "# Add\n1. **{{#0 + #1}}**\n\n# Mul\n1. **{{#0 * #1}}**\n").unwrap();

    // Main file calls two different blocks from the same import
    let main_source = "# Main\n1. [3, 4](lib#Add)\n2. [5, 6](lib#Mul)\n";
    let parser = mdl::parser::Parser::new(main_source.to_string(), 0);
    let program = parser.parse().expect("parse failed");
    let mut output = Vec::new();
    interpreter::execute_program_with_base(&program, &mut output, dir.path().to_path_buf())
        .map(|(_, _)| ())
        .expect("execution failed");
    let result = String::from_utf8(output).unwrap();
    // Both blocks print: Add prints 7, Mul prints 30
    assert_eq!(result.trim(), "7\n30");
}

#[test]
fn strikethrough_demand_arithmetic() {
    // Two-operand conditional produces Strikethrough; using it in arithmetic should demand-evaluate
    let src = r#"# M
1. x = true ? 5
2. **{x + 10}**"#;
    assert_eq!(run_trimmed(src), "15");
}

#[test]
fn strikethrough_demand_comparison() {
    // Demand-evaluate Strikethrough in comparison
    let src = r#"# M
1. x = true ? 10
2. **{x > 5}**"#;
    assert_eq!(run_trimmed(src), "true");
}

#[test]
fn strikethrough_falsy_stays_struck() {
    // False conditional produces Strikethrough, equality/logic don't demand
    let src = "# M\n1. x = false ? 42\n2. **{x == x}**";
    assert_eq!(run_trimmed(src), "true");
}

#[test]
fn table_block_body() {
    // A block with a table body (no chain) returns a Table value
    let src = r#"# Main
1. **{[](#Data)}**

## Data
| Name | Age |
|------|-----|
| Alice | 30 |
| Bob | 25 |"#;
    let output = run_trimmed(src);
    assert!(output.contains("Name"), "expected table with Name header: {}", output);
    assert!(output.contains("Alice"), "expected Alice in table: {}", output);
}

#[test]
fn comparison_gte_lte() {
    assert_eq!(run_trimmed("# M\n1. **{5 >= 5}**"), "true");
    assert_eq!(run_trimmed("# M\n1. **{5 >= 6}**"), "false");
    assert_eq!(run_trimmed("# M\n1. **{3 <= 3}**"), "true");
    assert_eq!(run_trimmed("# M\n1. **{4 <= 3}**"), "false");
}

#[test]
fn string_concatenation() {
    assert_eq!(
        run_trimmed(r#"# M
1. x = "hello" + " " + "world"
2. **{x}**"#),
        "hello world"
    );
}

#[test]
fn empty_block_returns_document() {
    // A block with no chain and no content returns an empty document
    let src = "# Main\n1. **{[](#Empty)}**\n\n## Empty\n";
    let output = run(src);
    // Should not crash, empty block returns empty string
    assert!(output.is_empty() || output.trim().is_empty(), "expected empty output: {:?}", output);
}

#[test]
fn recursive_factorial() {
    let src = r#"# Main
1. **{[5](#Fact)}**

## Fact
1. #0 <= 1 ? 1 : #0 * [#0 - 1](#Fact)"#;
    assert_eq!(run_trimmed(src), "120");
}

#[test]
fn spread_argument() {
    // Use spread ref in a non-bold context (assignment), then print the result
    let src = "# Main\n1. [42](#Echo)\n\n## Echo\n1. x = #*\n2. **{x}**";
    assert_eq!(run_trimmed(src), "[42]");
}

#[test]
fn print_interpolation() {
    let src = r#"# M
1. x = 5
2. y = 10
3. **{x} + {y} = {x + y}**"#;
    assert_eq!(run_trimmed(src), "5 + 10 = 15");
}

#[test]
fn match_alternation_first_arm() {
    let src = r#"# M
1. x = match 2
    - 1 | 2: "low"
    - 3 | 4 | 5: "mid"
    - otherwise: "high"
2. **{x}**"#;
    assert_eq!(run_trimmed(src), "low");
}

#[test]
fn match_alternation_second_arm() {
    let src = r#"# M
1. x = match 4
    - 1 | 2: "low"
    - 3 | 4 | 5: "mid"
    - otherwise: "high"
2. **{x}**"#;
    assert_eq!(run_trimmed(src), "mid");
}

#[test]
fn match_alternation_fallthrough_to_otherwise() {
    let src = r#"# M
1. x = match 99
    - 1 | 2: "low"
    - 3 | 4 | 5: "mid"
    - otherwise: "high"
2. **{x}**"#;
    assert_eq!(run_trimmed(src), "high");
}

#[test]
fn match_alternation_strings() {
    let src = r#"# M
1. x = match "b"
    - "a" | "b": "found"
    - otherwise: "nope"
2. **{x}**"#;
    assert_eq!(run_trimmed(src), "found");
}

#[test]
fn match_alternation_booleans() {
    let src = r#"# M
1. x = match true
    - true | false: "bool"
    - otherwise: "other"
2. **{x}**"#;
    assert_eq!(run_trimmed(src), "bool");
}
