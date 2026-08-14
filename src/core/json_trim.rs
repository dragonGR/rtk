//! JSON and Kubernetes API Payload Schema Trimmer.
//!
//! Automatically strips noisy, non-operational Kubernetes metadata fields (e.g. `managedFields`,
//! `ownerReferences`, `resourceVersion`, `uid`, `last-applied-configuration`) to reduce API payload
//! sizes by up to 90% before sending to LLM context windows.

#![allow(dead_code)]

use serde_json::Value;

/// Trims Kubernetes metadata and unnecessary payload bloat from a JSON string.
pub fn trim_k8s_json(json_str: &str) -> String {
    let Ok(mut val) = serde_json::from_str::<Value>(json_str) else {
        return json_str.to_string();
    };

    prune_k8s_value(&mut val);

    serde_json::to_string_pretty(&val).unwrap_or_else(|_| json_str.to_string())
}

fn prune_k8s_value(val: &mut Value) {
    match val {
        Value::Object(map) => {
            // Remove noisy metadata keys
            map.remove("managedFields");
            map.remove("ownerReferences");
            map.remove("resourceVersion");
            map.remove("generation");
            map.remove("uid");

            if let Some(Value::Object(meta)) = map.get_mut("metadata") {
                meta.remove("managedFields");
                meta.remove("ownerReferences");
                meta.remove("resourceVersion");
                meta.remove("generation");
                meta.remove("uid");

                if let Some(Value::Object(annotations)) = meta.get_mut("annotations") {
                    annotations.remove("kubectl.kubernetes.io/last-applied-configuration");
                }
            }

            // Recurse into child objects/arrays
            for (_, v) in map.iter_mut() {
                prune_k8s_value(v);
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                prune_k8s_value(item);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_k8s_json_strips_managed_fields() {
        let input = r#"{
  "apiVersion": "v1",
  "kind": "Pod",
  "metadata": {
    "name": "web-pod",
    "namespace": "default",
    "resourceVersion": "123456",
    "uid": "abc-123",
    "managedFields": [
      {
        "manager": "kubectl",
        "operation": "Update"
      }
    ]
  },
  "spec": {
    "containers": [
      {
        "name": "nginx",
        "image": "nginx:latest"
      }
    ]
  }
}"#;

        let trimmed = trim_k8s_json(input);
        assert!(trimmed.contains("web-pod"));
        assert!(trimmed.contains("nginx:latest"));
        assert!(!trimmed.contains("managedFields"));
        assert!(!trimmed.contains("resourceVersion"));
        assert!(!trimmed.contains("abc-123"));
    }
}
