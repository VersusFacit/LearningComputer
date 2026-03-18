use chrono::NaiveDate;

use crate::model::{DailyTask, Decision, P1Task, P2Task, P3Task, Snapshot};

const TOP_THREE_LIMIT: usize = 3;
const DAILY_STALE_DAYS: i64 = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Top3,
    P1,
    P2,
    P3,
    Daily,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetailMode {
    Closed,
    Item,
    Decisions,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DailyBucket {
    Active,
    Later,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DailyEntry<'a> {
    pub bucket: DailyBucket,
    pub task: &'a DailyTask,
    pub last_hit: Option<NaiveDate>,
    pub stale: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selected<'a> {
    P1(&'a P1Task),
    P2(&'a P2Task),
    P3(&'a P3Task),
    Daily(DailyEntry<'a>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Controller {
    snapshot: Snapshot,
    today: NaiveDate,
    pub screen: Screen,
    pub detail_mode: DetailMode,
    selections: Selections,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Selections {
    top3: usize,
    p1: usize,
    p2: usize,
    p3: usize,
    daily: usize,
}

impl Controller {
    pub fn new(snapshot: Snapshot, today: NaiveDate) -> Self {
        let mut controller = Self {
            snapshot,
            today,
            screen: Screen::Top3,
            detail_mode: DetailMode::Closed,
            selections: Selections::default(),
        };
        controller.clamp_all();
        controller
    }

    pub fn set_screen(&mut self, screen: Screen) {
        self.screen = screen;
        self.clamp(screen);
    }

    pub fn cycle_detail_mode(&mut self) {
        self.detail_mode = match self.detail_mode {
            DetailMode::Closed => DetailMode::Item,
            DetailMode::Item => DetailMode::Decisions,
            DetailMode::Decisions => DetailMode::Closed,
        };
    }

    pub fn selection(&self) -> usize {
        *self.selections.at(self.screen)
    }

    pub fn entry_count(&self) -> usize {
        self.len_for(self.screen)
    }

    pub fn select_next(&mut self) {
        let len = self.entry_count();
        if len > 0 {
            let selection = self.selections.at_mut(self.screen);
            *selection = (*selection + 1).min(len - 1);
        }
    }

    pub fn select_previous(&mut self) {
        let selection = self.selections.at_mut(self.screen);
        *selection = selection.saturating_sub(1);
    }

    pub fn select_first(&mut self) {
        *self.selections.at_mut(self.screen) = 0;
    }

    pub fn select_last(&mut self) {
        let len = self.entry_count();
        *self.selections.at_mut(self.screen) = len.saturating_sub(1);
    }

    pub fn top_three(&self) -> Vec<&P1Task> {
        let mut tasks: Vec<_> = self.snapshot.tasks.p1.iter().collect();
        tasks.sort_by_key(|task| task.rank);
        tasks.truncate(TOP_THREE_LIMIT);
        tasks
    }

    pub fn p1(&self) -> Vec<&P1Task> {
        let mut tasks: Vec<_> = self.snapshot.tasks.p1.iter().collect();
        tasks.sort_by_key(|task| task.rank);
        tasks
    }

    pub fn p2(&self) -> Vec<&P2Task> {
        let mut tasks: Vec<_> = self.snapshot.tasks.p2.iter().collect();
        tasks.sort_by_key(|task| task.source_order);
        tasks
    }

    pub fn p3(&self) -> Vec<&P3Task> {
        let mut tasks: Vec<_> = self.snapshot.tasks.p3.iter().collect();
        tasks.sort_by_key(|task| task.source_order);
        tasks
    }

    pub fn daily(&self) -> Vec<DailyEntry<'_>> {
        self.snapshot
            .dailies
            .active
            .iter()
            .map(|task| self.daily_entry(DailyBucket::Active, task))
            .chain(
                self.snapshot
                    .dailies
                    .later
                    .iter()
                    .map(|task| self.daily_entry(DailyBucket::Later, task)),
            )
            .collect()
    }

    pub fn selected(&self) -> Option<Selected<'_>> {
        match self.screen {
            Screen::Top3 => self
                .top_three()
                .get(self.selection())
                .copied()
                .map(Selected::P1),
            Screen::P1 => self.p1().get(self.selection()).copied().map(Selected::P1),
            Screen::P2 => self.p2().get(self.selection()).copied().map(Selected::P2),
            Screen::P3 => self.p3().get(self.selection()).copied().map(Selected::P3),
            Screen::Daily => self
                .daily()
                .get(self.selection())
                .copied()
                .map(Selected::Daily),
        }
    }

    pub fn decisions(&self) -> &[Decision] {
        &self.snapshot.decisions
    }

    pub fn replace_snapshot(&mut self, snapshot: Snapshot) {
        let saved = SavedSelections {
            top3: self.id_for(Screen::Top3),
            p1: self.id_for(Screen::P1),
            p2: self.id_for(Screen::P2),
            p3: self.id_for(Screen::P3),
            daily: self.id_for(Screen::Daily),
        };

        self.snapshot = snapshot;
        self.restore(Screen::Top3, saved.top3.as_deref());
        self.restore(Screen::P1, saved.p1.as_deref());
        self.restore(Screen::P2, saved.p2.as_deref());
        self.restore(Screen::P3, saved.p3.as_deref());
        self.restore(Screen::Daily, saved.daily.as_deref());
        self.clamp(self.screen);
    }

    fn daily_entry<'a>(&self, bucket: DailyBucket, task: &'a DailyTask) -> DailyEntry<'a> {
        let last_hit = task.hit_dates.iter().copied().max();
        let stale = last_hit
            .map(|last_hit| (self.today - last_hit).num_days() > DAILY_STALE_DAYS)
            .unwrap_or(true);

        DailyEntry {
            bucket,
            task,
            last_hit,
            stale,
        }
    }

    fn len_for(&self, screen: Screen) -> usize {
        match screen {
            Screen::Top3 => self.top_three().len(),
            Screen::P1 => self.snapshot.tasks.p1.len(),
            Screen::P2 => self.snapshot.tasks.p2.len(),
            Screen::P3 => self.snapshot.tasks.p3.len(),
            Screen::Daily => self.snapshot.dailies.active.len() + self.snapshot.dailies.later.len(),
        }
    }

    fn id_for(&self, screen: Screen) -> Option<String> {
        match screen {
            Screen::Top3 => self
                .top_three()
                .get(*self.selections.at(Screen::Top3))
                .map(|task| task.id.clone()),
            Screen::P1 => self
                .p1()
                .get(*self.selections.at(Screen::P1))
                .map(|task| task.id.clone()),
            Screen::P2 => self
                .p2()
                .get(*self.selections.at(Screen::P2))
                .map(|task| task.id.clone()),
            Screen::P3 => self
                .p3()
                .get(*self.selections.at(Screen::P3))
                .map(|task| task.id.clone()),
            Screen::Daily => self
                .daily()
                .get(*self.selections.at(Screen::Daily))
                .map(|entry| entry.task.id.clone()),
        }
    }

    fn restore(&mut self, screen: Screen, saved_id: Option<&str>) {
        let restored = saved_id.and_then(|saved_id| self.index_for(screen, saved_id));

        if let Some(index) = restored {
            *self.selections.at_mut(screen) = index;
        } else {
            self.clamp(screen);
        }
    }

    fn index_for(&self, screen: Screen, id: &str) -> Option<usize> {
        match screen {
            Screen::Top3 => self.top_three().iter().position(|task| task.id == id),
            Screen::P1 => self.p1().iter().position(|task| task.id == id),
            Screen::P2 => self.p2().iter().position(|task| task.id == id),
            Screen::P3 => self.p3().iter().position(|task| task.id == id),
            Screen::Daily => self.daily().iter().position(|entry| entry.task.id == id),
        }
    }

    fn clamp_all(&mut self) {
        self.clamp(Screen::Top3);
        self.clamp(Screen::P1);
        self.clamp(Screen::P2);
        self.clamp(Screen::P3);
        self.clamp(Screen::Daily);
    }

    fn clamp(&mut self, screen: Screen) {
        let len = self.len_for(screen);
        let selection = self.selections.at_mut(screen);
        *selection = if len == 0 {
            0
        } else {
            (*selection).min(len - 1)
        };
    }
}

impl Selections {
    fn at(&self, screen: Screen) -> &usize {
        match screen {
            Screen::Top3 => &self.top3,
            Screen::P1 => &self.p1,
            Screen::P2 => &self.p2,
            Screen::P3 => &self.p3,
            Screen::Daily => &self.daily,
        }
    }

    fn at_mut(&mut self, screen: Screen) -> &mut usize {
        match screen {
            Screen::Top3 => &mut self.top3,
            Screen::P1 => &mut self.p1,
            Screen::P2 => &mut self.p2,
            Screen::P3 => &mut self.p3,
            Screen::Daily => &mut self.daily,
        }
    }
}

#[derive(Default)]
struct SavedSelections {
    top3: Option<String>,
    p1: Option<String>,
    p2: Option<String>,
    p3: Option<String>,
    daily: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Snapshot;

    const FIXTURE: &str = r#"
schema_version: 1
captured_on: 2026-03-17
source_files: []
ingestion_rules: []
tasks:
  p1:
    - id: p1-003
      rank: 3
      status: todo
      title: Priority Item Gamma
      raw_text: Priority Item Gamma
      links: []
      notes: []
    - id: p1-001
      rank: 1
      status: todo
      title: Priority Item Alpha
      raw_text: Priority Item Alpha
      links: []
      notes: []
    - id: p1-004
      rank: 4
      status: todo
      title: Priority Item Delta
      raw_text: Priority Item Delta
      links: []
      notes: []
    - id: p1-002
      rank: 2
      status: todo
      title: Priority Item Beta
      raw_text: Priority Item Beta
      links: []
      notes: []
  p2:
    - id: p2-002
      source_order: 2
      status: todo
      title: Secondary Item Beta
      raw_text: Secondary Item Beta
      links: []
      notes: []
    - id: p2-001
      source_order: 1
      status: todo
      title: Secondary Item Alpha
      raw_text: Secondary Item Alpha
      links: []
      notes: []
  p3:
    - id: p3-002
      source_order: 2
      status: done
      title: Background Item Beta
      raw_text: Background Item Beta
      links: []
      notes: []
      completed_at: 2026-03-16
    - id: p3-001
      source_order: 1
      status: todo
      title: Background Item Alpha
      raw_text: Background Item Alpha
      links: []
      notes: []
dailies:
  active:
    - id: daily-001
      status: active
      title: Daily Topic Fresh
      raw_text: Daily Topic Fresh
      links: []
      notes: []
      hit_dates:
        - 2026-03-15
    - id: daily-002
      status: active
      title: Daily Topic Stale
      raw_text: Daily Topic Stale
      links: []
      notes: []
      hit_dates:
        - 2026-03-01
  later:
    - id: daily-003
      status: later
      title: Daily Topic Never
      raw_text: Daily Topic Never
      links: []
      notes: []
      hit_dates: []
session_state:
  active_work: []
  blocked: []
  daily_logs: []
decisions:
  - id: policy-001
    date: 2026-03-17
    title: Generic operating defaults
    settings: {}
    summary:
      - One summary line.
    startup_flow_notes:
      - One startup note.
"#;

    fn test_controller() -> Controller {
        let snapshot = Snapshot::from_yaml_str(FIXTURE).expect("controller fixture should parse");
        let today = NaiveDate::from_ymd_opt(2026, 3, 17).unwrap();
        Controller::new(snapshot, today)
    }

    #[test]
    fn derives_top_three_from_rank_order() {
        let controller = test_controller();
        let ids: Vec<_> = controller
            .top_three()
            .iter()
            .map(|task| task.id.as_str())
            .collect();

        assert_eq!(ids, vec!["p1-001", "p1-002", "p1-003"]);
    }

    #[test]
    fn keeps_selections_independent_per_screen() {
        let mut controller = test_controller();

        controller.set_screen(Screen::P1);
        controller.select_last();
        assert_eq!(controller.selection(), 3);

        controller.set_screen(Screen::P2);
        controller.select_next();
        assert_eq!(controller.selection(), 1);

        controller.set_screen(Screen::P1);
        assert_eq!(controller.selection(), 3);
    }

    #[test]
    fn clamps_selection_movement_to_bounds() {
        let mut controller = test_controller();

        controller.set_screen(Screen::P2);
        controller.select_previous();
        assert_eq!(controller.selection(), 0);

        controller.select_next();
        controller.select_next();
        assert_eq!(controller.selection(), 1);

        controller.select_first();
        assert_eq!(controller.selection(), 0);

        controller.select_last();
        assert_eq!(controller.selection(), 1);
    }

    #[test]
    fn cycles_detail_modes() {
        let mut controller = test_controller();

        assert_eq!(controller.detail_mode, DetailMode::Closed);
        controller.cycle_detail_mode();
        assert_eq!(controller.detail_mode, DetailMode::Item);
        controller.cycle_detail_mode();
        assert_eq!(controller.detail_mode, DetailMode::Decisions);
        controller.cycle_detail_mode();
        assert_eq!(controller.detail_mode, DetailMode::Closed);
    }

    #[test]
    fn derives_daily_last_hit_and_stale_state() {
        let controller = test_controller();
        let daily = controller.daily();

        assert_eq!(daily.len(), 3);
        assert_eq!(daily[0].bucket, DailyBucket::Active);
        assert_eq!(
            daily[0].last_hit,
            Some(NaiveDate::from_ymd_opt(2026, 3, 15).unwrap())
        );
        assert!(!daily[0].stale);

        assert_eq!(
            daily[1].last_hit,
            Some(NaiveDate::from_ymd_opt(2026, 3, 1).unwrap())
        );
        assert!(daily[1].stale);

        assert_eq!(daily[2].bucket, DailyBucket::Later);
        assert_eq!(daily[2].last_hit, None);
        assert!(daily[2].stale);
    }

    #[test]
    fn exposes_selected_data_for_rendering() {
        let mut controller = test_controller();

        match controller.selected() {
            Some(Selected::P1(task)) => assert_eq!(task.id, "p1-001"),
            _ => panic!("expected top-3 selection"),
        }

        controller.set_screen(Screen::Daily);
        controller.select_next();

        match controller.selected() {
            Some(Selected::Daily(entry)) => assert_eq!(entry.task.id, "daily-002"),
            _ => panic!("expected daily selection"),
        }
    }

    #[test]
    fn exposes_decisions_for_the_future_detail_panel() {
        let controller = test_controller();

        assert_eq!(controller.decisions().len(), 1);
        assert_eq!(controller.decisions()[0].id, "policy-001");
    }

    #[test]
    fn preserves_selected_ids_across_snapshot_replacement() {
        let mut controller = test_controller();
        controller.set_screen(Screen::P2);
        controller.select_next();

        let replacement = Snapshot::from_yaml_str(
            &FIXTURE.replace("Secondary Item Alpha", "Secondary Item Alpha Updated"),
        )
        .expect("replacement fixture should parse");

        controller.replace_snapshot(replacement);

        assert_eq!(controller.selection(), 1);
        match controller.selected() {
            Some(Selected::P2(task)) => assert_eq!(task.id, "p2-002"),
            _ => panic!("expected p2 selection"),
        }
    }
}
