use crate::commits::CommitSummary;

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
