use std::collections::BTreeMap;

use chrono::NaiveDate;
use serde::Deserialize;

use crate::error::ParseError;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Snapshot {
    pub schema_version: u32,
    pub captured_on: NaiveDate,
    #[serde(default)]
    pub source_files: Vec<String>,
    #[serde(default)]
    pub ingestion_rules: Vec<String>,
    pub tasks: Tasks,
    pub dailies: Dailies,
    pub session_state: SessionState,
    #[serde(default)]
    pub decisions: Vec<Decision>,
}

impl Snapshot {
    pub fn from_yaml_str(input: &str) -> Result<Self, ParseError> {
        Ok(serde_norway::from_str(input)?)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Tasks {
    #[serde(default)]
    pub p1: Vec<P1Task>,
    #[serde(default)]
    pub p2: Vec<P2Task>,
    #[serde(default)]
    pub p3: Vec<P3Task>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Dailies {
    #[serde(default)]
    pub active: Vec<DailyTask>,
    #[serde(default)]
    pub later: Vec<DailyTask>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct SessionState {
    #[serde(default)]
    pub active_work: Vec<String>,
    #[serde(default)]
    pub blocked: Vec<String>,
    #[serde(default)]
    pub daily_logs: Vec<DailyLog>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct DailyLog {
    pub date: NaiveDate,
    #[serde(default)]
    pub done: Vec<String>,
    #[serde(default)]
    pub in_progress: Vec<String>,
    #[serde(default)]
    pub blocked: Vec<String>,
    #[serde(default)]
    pub tomorrow: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Decision {
    pub id: String,
    pub date: NaiveDate,
    pub title: String,
    #[serde(default)]
    pub settings: BTreeMap<String, serde_norway::Value>,
    #[serde(default)]
    pub summary: Vec<String>,
    #[serde(default)]
    pub startup_flow_notes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Todo,
    Done,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DailyStatus {
    Active,
    Later,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct P1Task {
    pub id: String,
    pub rank: u32,
    pub status: TaskStatus,
    pub title: String,
    pub raw_text: String,
    #[serde(default)]
    pub links: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
    pub completed_at: Option<NaiveDate>,
    pub estimate_minutes_min: Option<u32>,
    pub estimate_minutes_max: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct P2Task {
    pub id: String,
    pub source_order: u32,
    pub status: TaskStatus,
    pub title: String,
    pub raw_text: String,
    #[serde(default)]
    pub links: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
    pub completed_at: Option<NaiveDate>,
    pub estimate_minutes_min: Option<u32>,
    pub estimate_minutes_max: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct P3Task {
    pub id: String,
    pub source_order: u32,
    pub status: TaskStatus,
    pub title: String,
    pub raw_text: String,
    #[serde(default)]
    pub links: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
    pub completed_at: Option<NaiveDate>,
    pub estimate_minutes_min: Option<u32>,
    pub estimate_minutes_max: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct DailyTask {
    pub id: String,
    pub status: DailyStatus,
    pub title: String,
    pub raw_text: String,
    #[serde(default)]
    pub links: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub hit_dates: Vec<NaiveDate>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_YAML: &str = r#"
schema_version: 1
captured_on: 2026-03-17
source_files:
  - /tmp/source.md
ingestion_rules:
  - Keep source material read-only during capture.
tasks:
  p1:
    - id: p1-001
      rank: 1
      status: todo
      title: Priority Item Alpha
      raw_text: Priority Item Alpha
      links: []
      notes: []
  p2:
    - id: p2-001
      source_order: 1
      status: todo
      title: Secondary Review Item
      raw_text: https://example.com/resource
      links:
        - https://example.com/resource
      notes: []
  p3:
    - id: p3-001
      source_order: 1
      status: done
      title: Background Exploration Item
      raw_text: Background Exploration Item
      links: []
      notes:
        - generic note
      completed_at: 2026-03-17
      estimate_minutes_min: 30
      estimate_minutes_max: 45
dailies:
  active:
    - id: daily-001
      status: active
      title: Daily Topic Alpha
      raw_text: Daily Topic Alpha
      links: []
      notes: []
      hit_dates:
        - 2026-03-10
  later:
    - id: later-daily-001
      status: later
      title: Deferred Topic Beta
      raw_text: Deferred Topic Beta
      links: []
      notes: []
      hit_dates: []
session_state:
  active_work:
    - Intake snapshot recorded for 2026-03-17
  blocked: []
  daily_logs:
    - date: 2026-03-17
      done: []
      in_progress:
        - Captured ranked tasks without mutating source material
      blocked: []
      tomorrow: []
decisions:
  - id: generic-policy-defaults
    date: 2026-03-17
    title: Generic operating defaults
    settings:
      policy.auto_apply: true
      policy.notify_on_change: true
    summary:
      - Enabled the default operating policy.
    startup_flow_notes:
      - Treat these settings as part of the default startup environment.
"#;

    #[test]
    fn parses_current_snapshot_shape() {
        let snapshot = Snapshot::from_yaml_str(VALID_YAML).expect("snapshot should parse");

        assert_eq!(snapshot.schema_version, 1);
        assert_eq!(snapshot.tasks.p1.len(), 1);
        assert_eq!(snapshot.tasks.p2.len(), 1);
        assert_eq!(snapshot.tasks.p3.len(), 1);
        assert_eq!(snapshot.dailies.active.len(), 1);
        assert_eq!(snapshot.dailies.later.len(), 1);
        assert_eq!(snapshot.session_state.daily_logs.len(), 1);
        assert_eq!(snapshot.decisions.len(), 1);
        assert_eq!(
            snapshot.tasks.p3[0].completed_at,
            Some(NaiveDate::from_ymd_opt(2026, 3, 17).unwrap())
        );
        assert_eq!(
            snapshot.dailies.active[0].hit_dates,
            vec![NaiveDate::from_ymd_opt(2026, 3, 10).unwrap()]
        );
    }

    #[test]
    fn tolerates_unknown_fields() {
        let yaml = r#"
schema_version: 1
captured_on: 2026-03-17
source_files: []
ingestion_rules: []
extra_top_level: true
tasks:
  p1:
    - id: p1-001
      rank: 1
      status: todo
      title: Priority Item Alpha
      raw_text: Priority Item Alpha
      links: []
      notes: []
      surprising: keep going
  p2: []
  p3: []
dailies:
  active: []
  later: []
session_state:
  active_work: []
  blocked: []
  daily_logs: []
decisions: []
"#;

        let snapshot = Snapshot::from_yaml_str(yaml).expect("unknown fields should be ignored");
        assert_eq!(snapshot.tasks.p1.len(), 1);
    }

    #[test]
    fn rejects_malformed_yaml() {
        let err =
            Snapshot::from_yaml_str("schema_version: [").expect_err("invalid YAML should fail");
        assert!(matches!(err, ParseError::InvalidYaml(_)));
    }

    #[test]
    fn rejects_invalid_status_values() {
        let yaml = VALID_YAML.replace("status: todo", "status: waiting");
        let err = Snapshot::from_yaml_str(&yaml).expect_err("invalid status should fail");
        assert!(matches!(err, ParseError::InvalidYaml(_)));
    }

    #[test]
    fn rejects_missing_required_fields() {
        let yaml = r#"
schema_version: 1
captured_on: 2026-03-17
source_files: []
ingestion_rules: []
dailies:
  active: []
  later: []
session_state:
  active_work: []
  blocked: []
  daily_logs: []
decisions: []
"#;

        let err = Snapshot::from_yaml_str(yaml).expect_err("missing tasks section should fail");
        assert!(matches!(err, ParseError::InvalidYaml(_)));
    }
}
