// SPDX-FileCopyrightText: 2026 Jay Taylor (https://github.com/otmof-ops/cig-compile)
// SPDX-License-Identifier: FSL-1.1-ALv2
//! A unified line diff, for `--passes-diff` (#33): what a pass changed in
//! the printed IR. The lines both sides share at the start and the end are
//! set aside first, so a pass that touches a few lines of a long function
//! costs the comparison of those few, not of the whole.

/// The unified diff of `before` and `after` with `context` lines around
/// each change, or `None` when they are the same.
pub fn unified(before: &str, after: &str, context: usize) -> Option<String> {
    if before == after {
        return None;
    }
    let a: Vec<&str> = before.lines().collect();
    let b: Vec<&str> = after.lines().collect();
    let mut head = 0;
    while head < a.len() && head < b.len() && a[head] == b[head] {
        head += 1;
    }
    let mut tail = 0;
    while tail < a.len() - head
        && tail < b.len() - head
        && a[a.len() - 1 - tail] == b[b.len() - 1 - tail]
    {
        tail += 1;
    }
    let (ma, mb) = (&a[head..a.len() - tail], &b[head..b.len() - tail]);
    // The longest common subsequence of the middles, by dynamic programming.
    let mut lcs = vec![vec![0u32; mb.len() + 1]; ma.len() + 1];
    for i in (0..ma.len()).rev() {
        for j in (0..mb.len()).rev() {
            lcs[i][j] = if ma[i] == mb[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    // Every line of both sides as kept, removed or added, in order.
    let mut ops: Vec<(char, &str)> = a[..head].iter().map(|l| (' ', *l)).collect();
    let (mut i, mut j) = (0, 0);
    while i < ma.len() || j < mb.len() {
        if i < ma.len() && j < mb.len() && ma[i] == mb[j] {
            ops.push((' ', ma[i]));
            i += 1;
            j += 1;
        } else if i < ma.len() && (j == mb.len() || lcs[i + 1][j] >= lcs[i][j + 1]) {
            // A removal before the addition that replaces it.
            ops.push(('-', ma[i]));
            i += 1;
        } else {
            ops.push(('+', mb[j]));
            j += 1;
        }
    }
    ops.extend(a[a.len() - tail..].iter().map(|l| (' ', *l)));
    // Hunks: each change with `context` kept lines around it, joined where
    // they meet.
    let changed: Vec<usize> = (0..ops.len()).filter(|&k| ops[k].0 != ' ').collect();
    let mut out = String::new();
    let mut k = 0;
    while k < changed.len() {
        let start = changed[k].saturating_sub(context);
        let mut end = changed[k];
        while k + 1 < changed.len() && changed[k + 1] <= end + 2 * context + 1 {
            k += 1;
            end = changed[k];
        }
        let end = (end + context + 1).min(ops.len());
        // Line numbers on each side where the hunk starts.
        let old_start = ops[..start].iter().filter(|o| o.0 != '+').count();
        let new_start = ops[..start].iter().filter(|o| o.0 != '-').count();
        let old_len = ops[start..end].iter().filter(|o| o.0 != '+').count();
        let new_len = ops[start..end].iter().filter(|o| o.0 != '-').count();
        out.push_str(&format!(
            "@@ -{},{old_len} +{},{new_len} @@\n",
            old_start + 1,
            new_start + 1
        ));
        for (op, line) in &ops[start..end] {
            out.push(*op);
            out.push_str(line);
            out.push('\n');
        }
        k += 1;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::unified;

    #[test]
    fn the_same_text_has_no_diff() {
        assert_eq!(unified("a\nb\n", "a\nb\n", 2), None);
    }

    #[test]
    fn a_change_is_said_with_its_context_and_line_numbers() {
        let before = "1\n2\n3\n4\n5\n6\n7\n8\n";
        let after = "1\n2\n3\nfour\n5\n6\n7\n8\n";
        assert_eq!(
            unified(before, after, 2).unwrap(),
            "@@ -2,5 +2,5 @@\n 2\n 3\n-4\n+four\n 5\n 6\n"
        );
    }

    #[test]
    fn changes_near_each_other_share_a_hunk_and_far_ones_do_not() {
        let before: String = (1..=20).map(|n| format!("{n}\n")).collect();
        let after: String = (1..=20)
            .filter(|n| *n != 5)
            .map(|n| match n {
                3 => "three\n".to_string(),
                18 => "x\n".to_string(),
                n => format!("{n}\n"),
            })
            .collect();
        let d = unified(&before, &after, 1).unwrap();
        assert_eq!(d.matches("@@").count(), 4, "two hunks: {d}");
        assert!(d.starts_with("@@ -2,5 +2,4 @@\n"), "{d}");
        assert!(
            d.contains("-3\n+three\n") && d.contains("-5\n") && d.contains("-18\n+x\n"),
            "{d}"
        );
    }

    #[test]
    fn lines_added_at_the_end_and_removed_at_the_start() {
        assert_eq!(
            unified("a\nb\n", "b\nc\n", 1).unwrap(),
            "@@ -1,2 +1,2 @@\n-a\n b\n+c\n"
        );
    }
}
