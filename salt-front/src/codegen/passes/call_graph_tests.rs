#[cfg(test)]
mod tests {
    use crate::codegen::passes::call_graph::*;

    // Helper: create a minimal analyzer with manual edges/attributes
    fn make_analyzer(
        edges: Vec<(&str, Vec<&str>)>,
        attrs: Vec<(&str, FnAttributes)>,
    ) -> CallGraphAnalyzer {
        let mut analyzer = CallGraphAnalyzer::new();
        for (name, callees) in edges {
            analyzer.call_edges.insert(
                name.to_string(),
                callees.into_iter().map(|s| s.to_string()).collect(),
            );
        }
        for (name, attr) in attrs {
            analyzer.fn_attributes.insert(name.to_string(), attr);
        }
        analyzer
    }

    #[test]
    fn test_direct_blocking_detection() {
        let mut analyzer = make_analyzer(
            vec![("read_data", vec!["TcpStream::read"])],
            vec![("read_data", FnAttributes::default())],
        );
        analyzer.run_propagation();
        assert!(analyzer.is_blocking("read_data"));
    }

    #[test]
    fn test_transitive_propagation() {
        // A calls B, B calls C, C is blocking
        let mut analyzer = make_analyzer(
            vec![
                ("A", vec!["B"]),
                ("B", vec!["C"]),
                ("C", vec!["TcpStream::read"]),
            ],
            vec![
                ("A", FnAttributes::default()),
                ("B", FnAttributes::default()),
                ("C", FnAttributes::default()),
            ],
        );
        analyzer.run_propagation();

        assert!(analyzer.is_blocking("C"), "C directly calls blocking op");
        assert!(analyzer.is_blocking("B"), "B transitively blocking via C");
        assert!(analyzer.is_blocking("A"), "A transitively blocking via B->C");
    }

    #[test]
    fn test_non_blocking_stays_clean() {
        let mut analyzer = make_analyzer(
            vec![
                ("pure_fn", vec!["math_add"]),
                ("math_add", vec![]),
            ],
            vec![
                ("pure_fn", FnAttributes::default()),
                ("math_add", FnAttributes::default()),
            ],
        );
        analyzer.run_propagation();

        assert!(!analyzer.is_blocking("pure_fn"));
        assert!(!analyzer.is_blocking("math_add"));
    }

    #[test]
    fn test_pulse_safety_violation() {
        let mut analyzer = make_analyzer(
            vec![
                ("update_ui", vec!["fetch_data"]),
                ("fetch_data", vec!["TcpStream::read"]),
            ],
            vec![
                ("update_ui", FnAttributes {
                    is_pulse: true,
                    pulse_hz: Some(60),
                    requires_context: true,
                    ..Default::default()
                }),
                ("fetch_data", FnAttributes::default()),
            ],
        );
        analyzer.run_propagation();
        let violations = analyzer.verify_pulse_safety_external();

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].pulse_fn, "update_ui");
    }

    #[test]
    fn test_fixed_point_convergence_on_cycle() {
        // Cyclic graph: A -> B -> C -> A (with C blocking)
        let mut analyzer = make_analyzer(
            vec![
                ("A", vec!["B"]),
                ("B", vec!["C"]),
                ("C", vec!["A", "Mutex::lock"]),
            ],
            vec![
                ("A", FnAttributes::default()),
                ("B", FnAttributes::default()),
                ("C", FnAttributes::default()),
            ],
        );
        analyzer.run_propagation();

        // All should be blocking because C reaches Mutex::lock
        assert!(analyzer.is_blocking("A"));
        assert!(analyzer.is_blocking("B"));
        assert!(analyzer.is_blocking("C"));
    }

    #[test]
    fn test_context_propagation() {
        let mut analyzer = make_analyzer(
            vec![
                ("handler", vec!["process"]),
                ("process", vec![]),
            ],
            vec![
                ("handler", FnAttributes::default()),
                ("process", FnAttributes {
                    requires_context: true,
                    is_yielding: true,
                    ..Default::default()
                }),
            ],
        );
        analyzer.run_propagation();

        assert!(analyzer.requires_context("handler"),
            "handler should inherit context requirement from process");
    }

    #[test]
    fn test_known_blocking_operations() {
        let analyzer = CallGraphAnalyzer::new();

        assert!(analyzer.is_known_blocking("TcpStream::read"));
        assert!(analyzer.is_known_blocking("fs::write"));
        assert!(analyzer.is_known_blocking("thread::sleep"));
        assert!(analyzer.is_known_blocking("Mutex::lock"));
        assert!(!analyzer.is_known_blocking("math::sqrt"));
        assert!(!analyzer.is_known_blocking("Vec::push"));
    }

    #[test]
    fn test_blocking_chain_bfs() {
        let mut analyzer = make_analyzer(
            vec![
                ("root", vec!["mid"]),
                ("mid", vec!["leaf"]),
                ("leaf", vec!["fs::open"]),
            ],
            vec![
                ("root", FnAttributes::default()),
                ("mid", FnAttributes::default()),
                ("leaf", FnAttributes::default()),
            ],
        );
        analyzer.run_propagation();

        let chain = analyzer.find_blocking_chain("root");
        assert!(chain.is_some());
        let chain = chain.expect("chain was verified Some above");
        assert_eq!(chain[0], "root");
        assert!(chain.len() >= 2, "Chain should trace through multiple hops");
    }

    #[test]
    fn test_empty_graph() {
        let mut analyzer = CallGraphAnalyzer::new();
        analyzer.run_propagation();
        let violations = analyzer.verify_pulse_safety_external();
        assert!(violations.is_empty());
    }
}
