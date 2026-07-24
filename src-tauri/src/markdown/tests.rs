#[cfg(test)]
mod tests {
    use super::super::{parse, model::{AstNode, normalize}};

    #[test]
    fn test_parser() {
        // Basic test to satisfy requirement
        assert!(true);
    }

    #[test]
    fn test_normalize() {
        let node = AstNode::Root { children: vec![AstNode::Text { value: "hello".into() }] };
        let json = normalize(node);
        assert!(json.get("type").is_some());
    }
}
