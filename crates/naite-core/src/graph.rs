use crate::commits::CommitSummary;
use crate::ops::history::RebaseAction;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphRow {
    /// Lane the commit's dot sits on.
    pub lane: u8,
    /// Lanes alive at the top edge of this row (i.e. continuing into it from above).
    /// If `lane` is in here, a line should be drawn from top to the dot.
    /// Lanes also in `lanes_out` should be drawn straight through.
    pub lanes_in: Vec<u8>,
    /// Lanes alive at the bottom edge of this row (continuing out below). Always
    /// equals the next row's `lanes_in`, which is what keeps adjacent cells visually connected.
    pub lanes_out: Vec<u8>,
    /// Lane each parent is placed on after this row. Used to draw the outgoing
    /// strokes from the dot down to the bottom edge.
    pub parent_lanes: Vec<u8>,
    pub total_lanes: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GraphLayout {
    pub rows: Vec<GraphRow>,
}

pub fn compute_graph_layout(commits: &[CommitSummary]) -> GraphLayout {
    // Each slot is a lane. `Some(id)` means the lane is waiting for commit `id`
    // to appear; `None` is a hole left behind by a merged branch and can be reused.
    let mut active: Vec<Option<String>> = Vec::new();
    let mut rows = Vec::with_capacity(commits.len());

    for commit in commits {
        let lanes_in = snapshot_active_lanes(&active);

        let lane = match active
            .iter()
            .position(|slot| slot.as_deref() == Some(commit.id.as_str()))
        {
            Some(found) => found,
            None => allocate_empty_lane(&mut active),
        };

        if lane < active.len() {
            active[lane] = None;
        }

        let mut parent_lanes = Vec::with_capacity(commit.parent_ids.len());
        for (parent_offset, parent) in commit.parent_ids.iter().enumerate() {
            if let Some(existing) = active
                .iter()
                .position(|slot| slot.as_deref() == Some(parent.as_str()))
            {
                parent_lanes.push(lane_to_u8(existing));
                continue;
            }

            let target = if parent_offset == 0 {
                if lane < active.len() {
                    active[lane] = Some(parent.clone());
                    lane
                } else {
                    active.push(Some(parent.clone()));
                    active.len() - 1
                }
            } else {
                place_in_empty_lane(&mut active, parent.clone())
            };
            parent_lanes.push(lane_to_u8(target));
        }

        while matches!(active.last(), Some(None)) {
            active.pop();
        }

        let lanes_out = snapshot_active_lanes(&active);

        let max_lane = lanes_in
            .iter()
            .chain(lanes_out.iter())
            .copied()
            .max()
            .unwrap_or(0) as usize;
        let total_lanes = max_lane
            .max(lane_to_u8(lane) as usize)
            .saturating_add(1)
            .min(u8::MAX as usize) as u8;

        rows.push(GraphRow {
            lane: lane_to_u8(lane),
            lanes_in,
            lanes_out,
            parent_lanes,
            total_lanes: total_lanes.max(1),
        });
    }

    GraphLayout { rows }
}

fn snapshot_active_lanes(active: &[Option<String>]) -> Vec<u8> {
    let mut lanes: Vec<u8> = active
        .iter()
        .enumerate()
        .filter_map(|(i, slot)| slot.as_ref().map(|_| lane_to_u8(i)))
        .collect();
    lanes.sort();
    lanes.dedup();
    lanes
}

fn allocate_empty_lane(active: &mut Vec<Option<String>>) -> usize {
    if let Some(empty) = active.iter().position(|slot| slot.is_none()) {
        empty
    } else {
        active.push(None);
        active.len() - 1
    }
}

fn place_in_empty_lane(active: &mut Vec<Option<String>>, value: String) -> usize {
    if let Some(empty) = active.iter().position(|slot| slot.is_none()) {
        active[empty] = Some(value);
        empty
    } else {
        active.push(Some(value));
        active.len() - 1
    }
}

fn lane_to_u8(lane: usize) -> u8 {
    lane.min(u8::MAX as usize) as u8
}

/// Returns `true` if `actions[index]` is a `Pick` whose commit message will be
/// rewritten when the rebase runs — i.e., at least one of the immediately
/// following consecutive children is a `Squash`. `Squash` opens git's message
/// editor with the combined messages of the pick and its squashes, which
/// effectively rewords the pick. A chain consisting only of `Fixup` does
/// not trigger this because fixup discards its own message and leaves the
/// pick's message untouched.
pub fn pick_inherits_reword(actions: &[RebaseAction], index: usize) -> bool {
    if actions.get(index).copied() != Some(RebaseAction::Pick) {
        return false;
    }
    actions[index + 1..]
        .iter()
        .copied()
        .take_while(|action| matches!(action, RebaseAction::Squash | RebaseAction::Fixup))
        .any(|action| action == RebaseAction::Squash)
}

/// Synthesizes a `GraphRow` per entry in an interactive-rebase plan so the
/// rebase editor can render squash/fixup grouping with the same renderer as
/// the commit list. Trunk actions (pick/reword/edit/drop) sit on lane 0;
/// squash/fixup drop to lane 1 and the trunk row above them spawns lane 1
/// as a "parent" so the canvas draws the elbow connecting them.
pub fn build_rebase_gutter(actions: &[RebaseAction]) -> Vec<GraphRow> {
    fn is_child(action: RebaseAction) -> bool {
        matches!(action, RebaseAction::Squash | RebaseAction::Fixup)
    }

    let mut rows = Vec::with_capacity(actions.len());
    for (i, &action) in actions.iter().enumerate() {
        let next_is_child = actions.get(i + 1).copied().is_some_and(is_child);
        let is_last = i + 1 == actions.len();
        let lane: u8 = if is_child(action) { 1 } else { 0 };

        let mut lanes_in: Vec<u8> = Vec::new();
        if i > 0 {
            lanes_in.push(0);
            if is_child(action) {
                lanes_in.push(1);
            }
        }

        let mut lanes_out: Vec<u8> = Vec::new();
        if !is_last {
            lanes_out.push(0);
            if next_is_child {
                lanes_out.push(1);
            }
        }

        let parent_lanes: Vec<u8> = if is_child(action) {
            if next_is_child {
                vec![1]
            } else {
                Vec::new()
            }
        } else if is_last {
            Vec::new()
        } else if next_is_child {
            vec![0, 1]
        } else {
            vec![0]
        };

        let max_lane = lanes_in
            .iter()
            .chain(lanes_out.iter())
            .copied()
            .max()
            .unwrap_or(lane)
            .max(lane);
        let total_lanes = max_lane.saturating_add(1).max(1);

        rows.push(GraphRow {
            lane,
            lanes_in,
            lanes_out,
            parent_lanes,
            total_lanes,
        });
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::*;

    #[test]
    fn graph_layout_handles_linear_history() {
        let commits = vec![
            commit("c3", &["c2"]),
            commit("c2", &["c1"]),
            commit("c1", &[]),
        ];

        let layout = compute_graph_layout(&commits);

        assert_eq!(layout.rows.len(), 3);
        assert_eq!(layout.rows[0].lane, 0);
        assert_eq!(layout.rows[1].lane, 0);
        assert_eq!(layout.rows[2].lane, 0);
    }

    #[test]
    fn graph_layout_allocates_merge_parent_lanes() {
        let commits = vec![
            commit("m", &["left", "right"]),
            commit("left", &["root"]),
            commit("right", &["root"]),
            commit("root", &[]),
        ];

        let layout = compute_graph_layout(&commits);

        assert_eq!(layout.rows[0].parent_lanes, vec![0, 1]);
        assert_eq!(layout.rows[0].lanes_out, vec![0, 1]);
        assert!(layout.rows.iter().any(|row| row.total_lanes >= 2));
    }

    #[test]
    fn graph_layout_links_each_row_to_the_next() {
        let commits = vec![
            commit("m", &["left", "right"]),
            commit("left", &["root"]),
            commit("right", &["root"]),
            commit("root", &[]),
        ];

        let layout = compute_graph_layout(&commits);

        for pair in layout.rows.windows(2) {
            assert_eq!(
                pair[0].lanes_out, pair[1].lanes_in,
                "row's outgoing lanes must equal the next row's incoming lanes"
            );
        }
    }

    #[test]
    fn graph_layout_marks_pass_through_lanes_for_branch_rows() {
        let commits = vec![
            commit("feature", &["root"]),
            commit("main", &["root"]),
            commit("root", &[]),
        ];

        let layout = compute_graph_layout(&commits);

        // `main` is on its own lane; lane 0 (carrying `root`) must pass through it
        // so the line from `feature` continues visually into the row beneath.
        let main_row = &layout.rows[1];
        assert_ne!(main_row.lane, 0);
        assert!(main_row.lanes_in.contains(&0));
        assert!(main_row.lanes_out.contains(&0));
        assert_eq!(main_row.parent_lanes, vec![0]);
    }

    #[test]
    fn graph_layout_keeps_branch_rows_distinguishable() {
        let commits = vec![
            commit("feature", &["root"]),
            commit("main", &["root"]),
            commit("root", &[]),
        ];

        let layout = compute_graph_layout(&commits);

        assert_eq!(layout.rows.len(), 3);
        assert_ne!(layout.rows[0].lane, layout.rows[1].lane);
        assert!(layout.rows.iter().any(|row| row.total_lanes >= 2));
    }

    #[test]
    fn rebase_gutter_empty_plan_produces_no_rows() {
        assert!(build_rebase_gutter(&[]).is_empty());
    }

    #[test]
    fn rebase_gutter_keeps_all_picks_on_trunk() {
        let rows =
            build_rebase_gutter(&[RebaseAction::Pick, RebaseAction::Pick, RebaseAction::Pick]);
        for row in &rows {
            assert_eq!(row.lane, 0);
            assert_eq!(row.total_lanes, 1);
        }
        assert_eq!(rows[0].lanes_in, Vec::<u8>::new());
        assert_eq!(rows[0].lanes_out, vec![0]);
        assert_eq!(rows[0].parent_lanes, vec![0]);
        assert_eq!(rows[1].lanes_in, vec![0]);
        assert_eq!(rows[1].lanes_out, vec![0]);
        assert_eq!(rows[2].lanes_out, Vec::<u8>::new());
        assert_eq!(rows[2].parent_lanes, Vec::<u8>::new());
    }

    #[test]
    fn rebase_gutter_spawns_sub_lane_for_squash_child() {
        let rows = build_rebase_gutter(&[RebaseAction::Pick, RebaseAction::Squash]);
        assert_eq!(rows[0].lane, 0);
        assert_eq!(rows[0].lanes_out, vec![0, 1]);
        assert_eq!(rows[0].parent_lanes, vec![0, 1]);
        assert_eq!(rows[1].lane, 1);
        assert_eq!(rows[1].lanes_in, vec![0, 1]);
        assert_eq!(rows[1].lanes_out, Vec::<u8>::new());
        assert_eq!(rows[1].parent_lanes, Vec::<u8>::new());
    }

    #[test]
    fn rebase_gutter_chains_consecutive_squash_siblings() {
        let rows = build_rebase_gutter(&[
            RebaseAction::Pick,
            RebaseAction::Squash,
            RebaseAction::Fixup,
            RebaseAction::Pick,
        ]);
        assert_eq!(rows[0].lanes_out, vec![0, 1]);
        assert_eq!(rows[0].parent_lanes, vec![0, 1]);
        assert_eq!(rows[1].lane, 1);
        assert_eq!(rows[1].lanes_in, vec![0, 1]);
        assert_eq!(rows[1].lanes_out, vec![0, 1]);
        assert_eq!(rows[1].parent_lanes, vec![1]);
        assert_eq!(rows[2].lane, 1);
        assert_eq!(rows[2].lanes_in, vec![0, 1]);
        assert_eq!(rows[2].lanes_out, vec![0]);
        assert_eq!(rows[2].parent_lanes, Vec::<u8>::new());
        assert_eq!(rows[3].lane, 0);
        assert_eq!(rows[3].lanes_in, vec![0]);
        assert_eq!(rows[3].lanes_out, Vec::<u8>::new());
    }

    #[test]
    fn rebase_gutter_keeps_trunk_continuous_across_drop() {
        let rows =
            build_rebase_gutter(&[RebaseAction::Pick, RebaseAction::Drop, RebaseAction::Pick]);
        assert_eq!(rows[1].lane, 0);
        assert_eq!(rows[1].lanes_in, vec![0]);
        assert_eq!(rows[1].lanes_out, vec![0]);
        assert_eq!(rows[1].parent_lanes, vec![0]);
    }

    #[test]
    fn pick_inherits_reword_only_when_a_squash_follows() {
        // pick at index 0 with a squash below → editor opens for combined message
        assert!(pick_inherits_reword(
            &[RebaseAction::Pick, RebaseAction::Squash],
            0
        ));
        // pick + fixup chain only → no editor, pick message untouched
        assert!(!pick_inherits_reword(
            &[RebaseAction::Pick, RebaseAction::Fixup, RebaseAction::Fixup],
            0
        ));
        // pick + fixup + squash → still rewritten (squash anywhere in chain triggers)
        assert!(pick_inherits_reword(
            &[
                RebaseAction::Pick,
                RebaseAction::Fixup,
                RebaseAction::Squash
            ],
            0
        ));
        // pick with no children at all
        assert!(!pick_inherits_reword(
            &[RebaseAction::Pick, RebaseAction::Pick],
            0
        ));
        // chain is bounded by the next non-child row — a later squash doesn't
        // attach to the earlier pick across a sibling pick
        assert!(!pick_inherits_reword(
            &[RebaseAction::Pick, RebaseAction::Pick, RebaseAction::Squash],
            0
        ));
        // second pick in the same plan does inherit reword from the squash that follows it
        assert!(pick_inherits_reword(
            &[RebaseAction::Pick, RebaseAction::Pick, RebaseAction::Squash],
            1
        ));
        // not a pick → never inherits
        assert!(!pick_inherits_reword(
            &[RebaseAction::Reword, RebaseAction::Squash],
            0
        ));
        assert!(!pick_inherits_reword(
            &[RebaseAction::Pick, RebaseAction::Squash],
            1
        ));
        // out-of-bounds index
        assert!(!pick_inherits_reword(&[RebaseAction::Pick], 5));
        assert!(!pick_inherits_reword(&[], 0));
    }

    #[test]
    fn rebase_gutter_links_each_row_to_the_next() {
        let rows = build_rebase_gutter(&[
            RebaseAction::Pick,
            RebaseAction::Squash,
            RebaseAction::Squash,
            RebaseAction::Pick,
            RebaseAction::Drop,
            RebaseAction::Reword,
        ]);
        for pair in rows.windows(2) {
            assert_eq!(
                pair[0].lanes_out, pair[1].lanes_in,
                "row's outgoing lanes must equal the next row's incoming lanes"
            );
        }
    }

    #[test]
    fn graph_layout_keeps_octopus_lanes_distinct() {
        let commits = vec![
            commit(
                "octopus",
                &["p1", "p2", "p3", "p4", "p5", "p6", "p7", "p8", "p9"],
            ),
            commit("p1", &[]),
            commit("p2", &[]),
            commit("p3", &[]),
            commit("p4", &[]),
            commit("p5", &[]),
            commit("p6", &[]),
            commit("p7", &[]),
            commit("p8", &[]),
            commit("p9", &[]),
        ];

        let layout = compute_graph_layout(&commits);

        assert_eq!(layout.rows[0].parent_lanes.len(), 9);
        assert_eq!(layout.rows[0].parent_lanes, vec![0, 1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(layout.rows[0].total_lanes, 9);
    }
}
