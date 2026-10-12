//! Dimensional constraints' expressions: numbers, + - * /, parentheses, names of other
//! parameters, sin/cos/tan (degrees), sqrt, abs and pi; a name depending on itself, through
//! others or straight, is refused.

use std::collections::BTreeMap;

use qiyas_param::{evaluate_all, Expr, ExprError};

fn params(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn numbers_operators_and_parentheses_follow_the_usual_order() {
    let none = |_: &str| None;
    let eval = |t: &str| Expr::parse(t).unwrap().eval(&none).unwrap();
    assert_eq!(eval("1.5"), 1.5);
    assert_eq!(eval("2 + 3 * 4"), 14.0);
    assert_eq!(eval("(2 + 3) * 4"), 20.0);
    assert_eq!(eval("10 / 4 - 1"), 1.5);
    assert_eq!(eval("-2 * -(3)"), 6.0);
    assert_eq!(eval("8 - 2 - 1"), 5.0);
    assert!((eval("sin(30)") - 0.5).abs() < 1e-12);
    assert!((eval("cos(60) + sqrt(16) + abs(-1)") - 5.5).abs() < 1e-12);
    assert!((eval("2*pi") - std::f64::consts::TAU).abs() < 1e-12);
    assert_eq!(eval("1e3"), 1000.0);
}

#[test]
fn bad_expressions_say_what_is_wrong() {
    for bad in ["", "2 +", "(1", "1)", "2 3", "foo(1)", "1 $ 2"] {
        assert!(
            matches!(Expr::parse(bad), Err(ExprError::Syntax(_))),
            "{bad:?} parsed"
        );
    }
    let none = |_: &str| None;
    assert!(matches!(
        Expr::parse("1/0").unwrap().eval(&none),
        Err(ExprError::NotANumber(_))
    ));
    assert_eq!(
        Expr::parse("2*d2 + d3").unwrap().names(),
        vec!["d2".to_string(), "d3".to_string()]
    );
}

#[test]
fn a_parameter_follows_the_ones_it_uses() {
    let values = evaluate_all(&params(&[("d1", "2*d2 + 1.5"), ("d2", "3")])).unwrap();
    assert_eq!(values["d1"], 7.5);
    let values = evaluate_all(&params(&[("d1", "2*d2 + 1.5"), ("d2", "5")])).unwrap();
    assert_eq!(values["d1"], 11.5);
    // A chain, in any order of names.
    let values = evaluate_all(&params(&[
        ("a", "b + 1"),
        ("b", "c * 2"),
        ("c", "d3"),
        ("d3", "4"),
    ]))
    .unwrap();
    assert_eq!(values["a"], 9.0);
}

#[test]
fn circular_and_unknown_references_are_refused() {
    let err = evaluate_all(&params(&[("d1", "d2 + 1"), ("d2", "d1 * 2")])).unwrap_err();
    assert!(
        matches!(err, ExprError::Circular(ref names) if names.contains(&"d1".to_string())),
        "{err:?}"
    );
    let err = evaluate_all(&params(&[("d1", "d1 + 1")])).unwrap_err();
    assert!(matches!(err, ExprError::Circular(_)), "{err:?}");
    let err = evaluate_all(&params(&[("d1", "d9 + 1")])).unwrap_err();
    assert_eq!(err, ExprError::Unknown("d9".into()));
}
