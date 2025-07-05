use mcp_client::{
    ClientCapabilities, Root, RootsCapability,
};

#[test]
fn test_roots_capability_serialization() {
    // Test that the roots capability serializes correctly
    let capabilities = ClientCapabilities {
        roots: Some(RootsCapability {
            list_changed: Some(true),
        }),
    };

    let json = serde_json::to_string(&capabilities).unwrap();
    assert!(json.contains("\"roots\""));
    assert!(json.contains("\"listChanged\":true"));
}

#[test]
fn test_root_serialization() {
    // Test root with name
    let root = Root {
        uri: "file:///home/user/projects".to_string(),
        name: Some("Projects".to_string()),
    };

    let json = serde_json::to_string(&root).unwrap();
    assert!(json.contains("\"uri\":\"file:///home/user/projects\""));
    assert!(json.contains("\"name\":\"Projects\""));

    // Test root without name
    let root_no_name = Root {
        uri: "file:///tmp".to_string(),
        name: None,
    };

    let json_no_name = serde_json::to_string(&root_no_name).unwrap();
    assert!(json_no_name.contains("\"uri\":\"file:///tmp\""));
    assert!(!json_no_name.contains("\"name\""));
}

#[test]
fn test_list_roots_result() {
    use mcp_client::client::ListRootsResult;
    
    let result = ListRootsResult {
        roots: vec![
            Root {
                uri: "file:///workspace".to_string(),
                name: Some("Workspace".to_string()),
            },
            Root {
                uri: "file:///documents".to_string(),
                name: None,
            },
        ],
    };

    let json = serde_json::to_string(&result).unwrap();
    let deserialized: ListRootsResult = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.roots.len(), 2);
    assert_eq!(deserialized.roots[0].uri, "file:///workspace");
    assert_eq!(deserialized.roots[0].name, Some("Workspace".to_string()));
    assert_eq!(deserialized.roots[1].uri, "file:///documents");
    assert_eq!(deserialized.roots[1].name, None);
} 