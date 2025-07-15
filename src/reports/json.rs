//! JSON format report generation

use serde_json::json;

use super::ReportGenerator;
use crate::detector::CycleDetector;
use crate::error::FerrisWheelError;

pub struct JsonReportGenerator;

impl Default for JsonReportGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonReportGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl ReportGenerator for JsonReportGenerator {
    fn generate_report(&self, detector: &CycleDetector) -> Result<String, FerrisWheelError> {
        let mut cycles: Vec<_> = detector
            .cycles()
            .iter()
            .map(|cycle| {
                let mut workspace_names = cycle.workspace_names().to_vec();
                workspace_names.sort();

                let mut edges: Vec<_> = cycle
                    .edges()
                    .iter()
                    .map(|edge| {
                        json!({
                            "from_crate": edge.from_crate(),
                            "to_crate": edge.to_crate(),
                            "dependency_type": edge.dependency_type(),
                        })
                    })
                    .collect();

                // Sort edges by from_crate, then to_crate for consistent ordering
                edges.sort_by(|a, b| {
                    let a_from = a["from_crate"].as_str().unwrap_or("");
                    let b_from = b["from_crate"].as_str().unwrap_or("");
                    match a_from.cmp(b_from) {
                        std::cmp::Ordering::Equal => {
                            let a_to = a["to_crate"].as_str().unwrap_or("");
                            let b_to = b["to_crate"].as_str().unwrap_or("");
                            a_to.cmp(b_to)
                        }
                        other => other,
                    }
                });

                json!({
                    "workspaces": workspace_names,
                    "edges": edges
                })
            })
            .collect();

        // Sort cycles by their first workspace name for consistent ordering
        cycles.sort_by(|a, b| {
            let a_workspaces = a["workspaces"].as_array();
            let b_workspaces = b["workspaces"].as_array();
            let a_first = a_workspaces
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let b_first = b_workspaces
                .and_then(|arr| arr.first())
                .and_then(|v| v.as_str())
                .unwrap_or("");
            a_first.cmp(b_first)
        });

        let report = json!({
            "has_cycles": detector.has_cycles(),
            "cycle_count": detector.cycle_count(),
            "cycles": cycles,
        });

        serde_json::to_string_pretty(&report).map_err(FerrisWheelError::Json)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::detector::{CycleDetector, WorkspaceCycle};

    fn create_test_detector_with_cycles() -> CycleDetector {
        let mut detector = CycleDetector::new();

        // Create a simple cycle: workspace-a -> workspace-b -> workspace-a
        let cycle = WorkspaceCycle::builder()
            .with_workspace_names(vec!["workspace-a".to_string(), "workspace-b".to_string()])
            .add_edge()
            .from_workspace("workspace-a")
            .to_workspace("workspace-b")
            .from_crate("crate-a")
            .to_crate("crate-b")
            .dependency_type("normal")
            .add_edge()
            .expect("Failed to add first edge")
            .from_workspace("workspace-b")
            .to_workspace("workspace-a")
            .from_crate("crate-b")
            .to_crate("crate-a")
            .dependency_type("dev")
            .build()
            .expect("Failed to build cycle");

        detector.add_cycle(cycle);
        detector
    }

    #[test]
    fn test_json_report_no_cycles() {
        let detector = CycleDetector::new();
        let generator = JsonReportGenerator::new();

        let report = generator.generate_report(&detector).unwrap();
        let json: Value = serde_json::from_str(&report).unwrap();

        assert_eq!(json["has_cycles"], false);
        assert_eq!(json["cycle_count"], 0);
        assert_eq!(json["cycles"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_json_report_with_cycles() {
        let detector = create_test_detector_with_cycles();
        let generator = JsonReportGenerator::new();

        let report = generator.generate_report(&detector).unwrap();
        let json: Value = serde_json::from_str(&report).unwrap();

        assert_eq!(json["has_cycles"], true);
        assert_eq!(json["cycle_count"], 1);

        let cycles = json["cycles"].as_array().unwrap();
        assert_eq!(cycles.len(), 1);

        let cycle = &cycles[0];
        let workspaces = cycle["workspaces"].as_array().unwrap();
        assert_eq!(workspaces.len(), 2);
        assert!(workspaces.contains(&json!("workspace-a")));
        assert!(workspaces.contains(&json!("workspace-b")));

        let edges = cycle["edges"].as_array().unwrap();
        assert_eq!(edges.len(), 2);
    }

    #[test]
    fn test_json_report_edge_structure() {
        let detector = create_test_detector_with_cycles();
        let generator = JsonReportGenerator::new();

        let report = generator.generate_report(&detector).unwrap();
        let json: Value = serde_json::from_str(&report).unwrap();

        let edge = &json["cycles"][0]["edges"][0];
        assert!(edge.get("from_crate").is_some());
        assert!(edge.get("to_crate").is_some());
        assert!(edge.get("dependency_type").is_some());
    }

    #[test]
    fn test_json_report_pretty_formatting() {
        let detector = CycleDetector::new();
        let generator = JsonReportGenerator::new();

        let report = generator.generate_report(&detector).unwrap();

        // Pretty formatted JSON should have newlines and indentation
        assert!(report.contains('\n'));
        assert!(report.contains("  "));
    }

    #[test]
    fn test_json_report_default_trait() {
        let generator1 = JsonReportGenerator;
        let generator2 = JsonReportGenerator::new();

        // Both should produce the same results
        let detector = CycleDetector::new();
        let report1 = generator1.generate_report(&detector).unwrap();
        let report2 = generator2.generate_report(&detector).unwrap();

        assert_eq!(report1, report2);
    }
}
