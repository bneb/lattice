#[test]
fn test_repro_all_types() {
    let code = r#"
fn use_types(
    a: u32, 
    b: u64, 
    c: f32, 
    d: usize,
    e: bool,
    f: i8
) -> f64 {
    let x: f64 = 1.0;
    return x;
}

struct Data {
    x: u32,
    y: f32,
}

fn main() {
    let d = Data { x: 10, y: 2.5 as f32 };
    let res = use_types(d.x, 100, d.y, 0, true, 1);
}
    "#;
    let res = saltc::compile(code, false, None, true);
    assert!(res.is_ok());
    println!("{}", res.unwrap());
}
