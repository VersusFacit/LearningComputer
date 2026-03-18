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
    Decisions,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetailMode {
    Closed,
    Item,
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
    Decision(&'a Decision),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Controller {
    snapshot: Snapshot,
    today: NaiveDate,
    pub screen: Screen,
    pub detail_mode: DetailMode,
    derived: Derived,
    selections: Selections,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Selections {
    top3: usize,
    p1: usize,
    p2: usize,
    p3: usize,
    daily: usize,
    decisions: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Derived {
    p1_order: Vec<usize>,
    top3_order: Vec<usize>,
    daily: Vec<DailyDerived>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DailyDerived {
    bucket: DailyBucket,
    active: bool,
    task_index: usize,
    last_hit: Option<NaiveDate>,
    stale: bool,
}

impl Controller {
    pub fn new(snapshot: Snapshot, today: NaiveDate) -> Self {
        let derived = Derived::build(&snapshot, today);
        let mut controller = Self {
            snapshot,
            today,
            screen: Screen::Top3,
            detail_mode: DetailMode::Closed,
            derived,
            selections: Selections::default(),
        };
        controller.repair_all_selections();
        controller
    }

    pub fn set_screen(&mut self, screen: Screen) {
        self.screen = screen;
        self.repair_selection(screen);
    }

    pub fn cycle_detail_mode(&mut self) {
        self.detail_mode = match self.detail_mode {
            DetailMode::Closed => DetailMode::Item,
            DetailMode::Item => DetailMode::Closed,
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
            *selection = (*selection + 1) % len;
        }
    }

    pub fn select_previous(&mut self) {
        let len = self.entry_count();
        if len > 0 {
            let selection = self.selections.at_mut(self.screen);
            *selection = if *selection == 0 {
                len - 1
            } else {
                *selection - 1
            };
        }
    }

    pub fn select_first(&mut self) {
        *self.selections.at_mut(self.screen) = 0;
    }

    pub fn select_last(&mut self) {
        let len = self.entry_count();
        *self.selections.at_mut(self.screen) = len.saturating_sub(1);
    }

    pub fn top_three(&self) -> impl Iterator<Item = &P1Task> + '_ {
        self.derived
            .top3_order
            .iter()
            .filter_map(|&index| self.snapshot.tasks.p1.get(index))
    }

    pub fn p1(&self) -> impl Iterator<Item = &P1Task> + '_ {
        self.derived
            .p1_order
            .iter()
            .filter_map(|&index| self.snapshot.tasks.p1.get(index))
    }

    pub fn p2(&self) -> &[P2Task] {
        &self.snapshot.tasks.p2
    }

    pub fn p3(&self) -> &[P3Task] {
        &self.snapshot.tasks.p3
    }

    pub fn daily(&self) -> impl Iterator<Item = DailyEntry<'_>> + '_ {
        self.derived
            .daily
            .iter()
            .filter_map(|entry| self.daily_from(entry))
    }

    pub fn selected(&self) -> Option<Selected<'_>> {
        self.entry_at(self.screen, self.selection())
    }

    pub fn decisions(&self) -> &[Decision] {
        &self.snapshot.decisions
    }

    pub fn captured_on(&self) -> NaiveDate {
        self.snapshot.captured_on
    }

    pub fn replace_snapshot(&mut self, snapshot: Snapshot) {
        let restore_state = SelectionRestoreState {
            top3_selected_id: self.id_for(Screen::Top3),
            p1_selected_id: self.id_for(Screen::P1),
            p2_selected_id: self.id_for(Screen::P2),
            p3_selected_id: self.id_for(Screen::P3),
            daily_selected_id: self.id_for(Screen::Daily),
            decisions_selected_id: self.id_for(Screen::Decisions),
        };

        self.snapshot = snapshot;
        self.derived = Derived::build(&self.snapshot, self.today);
        self.restore(Screen::Top3, restore_state.top3_selected_id.as_deref());
        self.restore(Screen::P1, restore_state.p1_selected_id.as_deref());
        self.restore(Screen::P2, restore_state.p2_selected_id.as_deref());
        self.restore(Screen::P3, restore_state.p3_selected_id.as_deref());
        self.restore(Screen::Daily, restore_state.daily_selected_id.as_deref());
        self.restore(
            Screen::Decisions,
            restore_state.decisions_selected_id.as_deref(),
        );
        self.repair_selection(self.screen);
    }

    fn entry_at(&self, screen: Screen, index: usize) -> Option<Selected<'_>> {
        match screen {
            Screen::Top3 => self.top_three_at(index).map(Selected::P1),
            Screen::P1 => self.p1_at(index).map(Selected::P1),
            Screen::P2 => self.p2().get(index).map(Selected::P2),
            Screen::P3 => self.p3().get(index).map(Selected::P3),
            Screen::Daily => self.daily_at(index).map(Selected::Daily),
            Screen::Decisions => self.decisions().get(index).map(Selected::Decision),
        }
    }

    fn top_three_at(&self, index: usize) -> Option<&P1Task> {
        self.derived
            .top3_order
            .get(index)
            .and_then(|&task_index| self.snapshot.tasks.p1.get(task_index))
    }

    fn p1_at(&self, index: usize) -> Option<&P1Task> {
        self.derived
            .p1_order
            .get(index)
            .and_then(|&task_index| self.snapshot.tasks.p1.get(task_index))
    }

    fn daily_at(&self, index: usize) -> Option<DailyEntry<'_>> {
        self.derived
            .daily
            .get(index)
            .and_then(|entry| self.daily_from(entry))
    }

    fn daily_from(&self, entry: &DailyDerived) -> Option<DailyEntry<'_>> {
        let task = if entry.active {
            self.snapshot.dailies.active.get(entry.task_index)
        } else {
            self.snapshot.dailies.later.get(entry.task_index)
        }?;

        Some(DailyEntry {
            bucket: entry.bucket,
            task,
            last_hit: entry.last_hit,
            stale: entry.stale,
        })
    }

    fn len_for(&self, screen: Screen) -> usize {
        match screen {
            Screen::Top3 => self.derived.top3_order.len(),
            Screen::P1 => self.derived.p1_order.len(),
            Screen::P2 => self.snapshot.tasks.p2.len(),
            Screen::P3 => self.snapshot.tasks.p3.len(),
            Screen::Daily => self.derived.daily.len(),
            Screen::Decisions => self.snapshot.decisions.len(),
        }
    }

    /// Capture the currently selected item's stable id before replacing the
    /// snapshot.
    ///
    /// Selection indices are only positions within the current derived lists.
    /// A reload can insert, remove, or reorder items, which makes the old
    /// numeric index point at the wrong logical item. We clone the selected id
    /// here so `restore` can find the same logical item in the new snapshot
    /// after replacement.
    fn id_for(&self, screen: Screen) -> Option<String> {
        self.entry_at(screen, *self.selections.at(screen))
            .map(selected_id)
    }

    fn restore(&mut self, screen: Screen, saved_id: Option<&str>) {
        match saved_id.and_then(|saved_id| self.index_for(screen, saved_id)) {
            Some(index) => *self.selections.at_mut(screen) = index,
            None => self.repair_selection(screen),
        }
    }

    fn index_for(&self, screen: Screen, id: &str) -> Option<usize> {
        let len = self.len_for(screen);
        (0..len).position(|index| {
            self.entry_at(screen, index)
                .is_some_and(|selected| selected_id(selected) == id)
        })
    }

    fn repair_all_selections(&mut self) {
        self.repair_selection(Screen::Top3);
        self.repair_selection(Screen::P1);
        self.repair_selection(Screen::P2);
        self.repair_selection(Screen::P3);
        self.repair_selection(Screen::Daily);
        self.repair_selection(Screen::Decisions);
    }

    /// Repair a stored selection after the underlying data changes.
    ///
    /// Example: the user had item index `7` selected, then a reload replaces the
    /// snapshot and that view now has only `3` rows. The navigation behavior
    /// should still wrap during normal movement, but after a reload we need to
    /// bring the stored index back into bounds so the future UI can safely ask
    /// for the selected row without defending against invalid state.
    fn repair_selection(&mut self, screen: Screen) {
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
            Screen::Decisions => &self.decisions,
        }
    }

    fn at_mut(&mut self, screen: Screen) -> &mut usize {
        match screen {
            Screen::Top3 => &mut self.top3,
            Screen::P1 => &mut self.p1,
            Screen::P2 => &mut self.p2,
            Screen::P3 => &mut self.p3,
            Screen::Daily => &mut self.daily,
            Screen::Decisions => &mut self.decisions,
        }
    }
}

impl Derived {
    fn build(snapshot: &Snapshot, today: NaiveDate) -> Self {
        let mut p1_order: Vec<_> = (0..snapshot.tasks.p1.len()).collect();
        p1_order.sort_by_key(|&index| snapshot.tasks.p1[index].rank);

        let top3_order = p1_order.iter().take(TOP_THREE_LIMIT).copied().collect();

        let daily = snapshot
            .dailies
            .active
            .iter()
            .enumerate()
            .map(|(task_index, task)| {
                DailyDerived::new(DailyBucket::Active, true, task_index, task, today)
            })
            .chain(
                snapshot
                    .dailies
                    .later
                    .iter()
                    .enumerate()
                    .map(|(task_index, task)| {
                        DailyDerived::new(DailyBucket::Later, false, task_index, task, today)
                    }),
            )
            .collect();

        Self {
            p1_order,
            top3_order,
            daily,
        }
    }
}

impl DailyDerived {
    fn new(
        bucket: DailyBucket,
        active: bool,
        task_index: usize,
        task: &DailyTask,
        today: NaiveDate,
    ) -> Self {
        let last_hit = task.hit_dates.iter().copied().max();
        let stale = last_hit
            .map(|last_hit| (today - last_hit).num_days() > DAILY_STALE_DAYS)
            .unwrap_or(true);

        Self {
            bucket,
            active,
            task_index,
            last_hit,
            stale,
        }
    }
}

fn selected_id(selected: Selected<'_>) -> String {
    match selected {
        Selected::P1(task) => task.id.clone(),
        Selected::P2(task) => task.id.clone(),
        Selected::P3(task) => task.id.clone(),
        Selected::Daily(entry) => entry.task.id.clone(),
        Selected::Decision(decision) => decision.id.clone(),
    }
}

#[derive(Default)]
struct SelectionRestoreState {
    top3_selected_id: Option<String>,
    p1_selected_id: Option<String>,
    p2_selected_id: Option<String>,
    p3_selected_id: Option<String>,
    daily_selected_id: Option<String>,
    decisions_selected_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Snapshot;

    fn test_controller() -> Controller {
        let snapshot = Snapshot::from_yaml_str(FIXTURE).expect("controller fixture should parse");
        let today = NaiveDate::from_ymd_opt(2026, 3, 17).unwrap();
        Controller::new(snapshot, today)
    }

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

    #[test]
    fn derives_top_three_from_rank_order_once() {
        let controller = test_controller();
        let ids: Vec<_> = controller
            .top_three()
            .map(|task| task.id.as_str())
            .collect();

        assert_eq!(ids, vec!["p1-001", "p1-002", "p1-003"]);
    }

    #[test]
    fn exposes_p1_in_rank_order() {
        let controller = test_controller();
        let ids: Vec<_> = controller.p1().map(|task| task.id.as_str()).collect();

        assert_eq!(ids, vec!["p1-001", "p1-002", "p1-003", "p1-004"]);
    }

    #[test]
    fn keeps_p2_and_p3_in_source_order() {
        let controller = test_controller();
        let p2_ids: Vec<_> = controller
            .p2()
            .iter()
            .map(|task| task.id.as_str())
            .collect();
        let p3_ids: Vec<_> = controller
            .p3()
            .iter()
            .map(|task| task.id.as_str())
            .collect();

        assert_eq!(p2_ids, vec!["p2-002", "p2-001"]);
        assert_eq!(p3_ids, vec!["p3-002", "p3-001"]);
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
    fn wraps_selection_movement() {
        let mut controller = test_controller();

        controller.set_screen(Screen::P2);
        controller.select_previous();
        assert_eq!(controller.selection(), 1);

        controller.select_next();
        assert_eq!(controller.selection(), 0);

        controller.select_last();
        controller.select_next();
        assert_eq!(controller.selection(), 0);

        controller.select_first();
        assert_eq!(controller.selection(), 0);
    }

    #[test]
    fn cycles_detail_modes() {
        let mut controller = test_controller();

        assert_eq!(controller.detail_mode, DetailMode::Closed);
        controller.cycle_detail_mode();
        assert_eq!(controller.detail_mode, DetailMode::Item);
        controller.cycle_detail_mode();
        assert_eq!(controller.detail_mode, DetailMode::Closed);
    }

    #[test]
    fn derives_daily_last_hit_and_stale_state_once() {
        let controller = test_controller();
        let daily: Vec<_> = controller.daily().collect();

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
            _ => panic!("expected top-three selection"),
        }

        controller.set_screen(Screen::Daily);
        controller.select_next();

        match controller.selected() {
            Some(Selected::Daily(entry)) => assert_eq!(entry.task.id, "daily-002"),
            _ => panic!("expected daily selection"),
        }
    }

    #[test]
    fn exposes_decisions_for_the_decisions_screen() {
        let controller = test_controller();

        assert_eq!(controller.decisions().len(), 1);
        assert_eq!(controller.decisions()[0].id, "policy-001");
    }

    #[test]
    fn exposes_selected_decision_for_rendering() {
        let mut controller = test_controller();
        controller.set_screen(Screen::Decisions);

        match controller.selected() {
            Some(Selected::Decision(decision)) => assert_eq!(decision.id, "policy-001"),
            _ => panic!("expected decision selection"),
        }
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
            Some(Selected::P2(task)) => assert_eq!(task.id, "p2-001"),
            _ => panic!("expected p2 selection"),
        }
    }
}
