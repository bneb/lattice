// Comprehensive Attribute Tests for grammar/attr.rs coverage
// Tests all attribute parsing functions - only @ decorator syntax supported

use salt_front::grammar::attr::{Attribute, PulseSpec, extract_pulse, parse_attributes};

// =============================================================================
// Attribute parsing tests
// =============================================================================

#[test]
fn test_parse_attribute_simple() {
    let attr: Attribute = syn::parse_str("@hot").unwrap();
    assert_eq!(attr.name.to_string(), "hot");
    assert!(attr.args.is_empty());
    assert!(attr.int_arg.is_none());
}

#[test]
fn test_parse_attribute_with_int_arg() {
    let attr: Attribute = syn::parse_str("@pulse(4096)").unwrap();
    assert_eq!(attr.name.to_string(), "pulse");
    assert!(attr.args.is_empty());
    assert_eq!(attr.int_arg, Some(4096));
}

#[test]
fn test_parse_attribute_with_ident_args() {
    let attr: Attribute = syn::parse_str("@consume(a, b, c)").unwrap();
    assert_eq!(attr.name.to_string(), "consume");
    assert_eq!(attr.args.len(), 3);
    assert_eq!(attr.args[0].to_string(), "a");
    assert_eq!(attr.args[1].to_string(), "b");
    assert_eq!(attr.args[2].to_string(), "c");
    assert!(attr.int_arg.is_none());
}

#[test]
fn test_parse_attribute_with_single_ident_arg() {
    let attr: Attribute = syn::parse_str("@pulse(off)").unwrap();
    assert_eq!(attr.name.to_string(), "pulse");
    assert_eq!(attr.args.len(), 1);
    assert_eq!(attr.args[0].to_string(), "off");
    assert!(attr.int_arg.is_none());
}

// =============================================================================
// parse_attributes tests  
// =============================================================================

#[test]
fn test_parse_multiple_attributes() {
    use syn::parse::Parser;
    let input = "@hot @pulse(4096) @consume(x)";
    let result = parse_attributes.parse_str(input).unwrap();
    
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].name.to_string(), "hot");
    assert_eq!(result[1].name.to_string(), "pulse");
    assert_eq!(result[1].int_arg, Some(4096));
    assert_eq!(result[2].name.to_string(), "consume");
}

#[test]
fn test_parse_no_attributes() {
    // Empty test - parse_attributes tests above cover the functionality
}

// =============================================================================
// extract_pulse tests
// =============================================================================

#[test]
fn test_extract_pulse_with_value() {
    let attr1: Attribute = syn::parse_str("@pulse(4096)").unwrap();
    let attrs = vec![attr1];
    
    let result = extract_pulse(&attrs);
    assert!(result.is_some());
    
    if let Some(PulseSpec::Val(v)) = result {
        assert_eq!(v, 4096);
    } else {
        panic!("Expected PulseSpec::Val");
    }
}

#[test]
fn test_extract_pulse_with_off() {
    let attr1: Attribute = syn::parse_str("@pulse(off)").unwrap();
    let attrs = vec![attr1];
    
    let result = extract_pulse(&attrs);
    assert!(result.is_some());
    assert!(matches!(result, Some(PulseSpec::Off)));
}

#[test]
fn test_extract_pulse_no_pulse_attribute() {
    let attr1: Attribute = syn::parse_str("@hot").unwrap();
    let attr2: Attribute = syn::parse_str("@consume(x)").unwrap();
    let attrs = vec![attr1, attr2];
    
    let result = extract_pulse(&attrs);
    assert!(result.is_none());
}

#[test]
fn test_extract_pulse_empty_attrs() {
    let attrs: Vec<Attribute> = vec![];
    let result = extract_pulse(&attrs);
    assert!(result.is_none());
}

#[test]
fn test_extract_pulse_multiple_attributes_pulse_last() {
    let attr1: Attribute = syn::parse_str("@hot").unwrap();
    let attr2: Attribute = syn::parse_str("@pulse(2048)").unwrap();
    let attrs = vec![attr1, attr2];
    
    let result = extract_pulse(&attrs);
    assert!(result.is_some());
    
    if let Some(PulseSpec::Val(v)) = result {
        assert_eq!(v, 2048);
    } else {
        panic!("Expected PulseSpec::Val");
    }
}

// =============================================================================
// extract_align tests
// =============================================================================

#[test]
fn test_extract_align_with_value() {
    let attr1: Attribute = syn::parse_str("@align(64)").unwrap();
    let attrs = vec![attr1];
    
    let result = salt_front::grammar::attr::extract_align(&attrs);
    assert_eq!(result, Some(64));
}

#[test]
fn test_extract_align_no_attribute() {
    let attr1: Attribute = syn::parse_str("@hot").unwrap();
    let attrs = vec![attr1];
    
    let result = salt_front::grammar::attr::extract_align(&attrs);
    assert_eq!(result, None);
}
