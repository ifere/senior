#[derive(Debug, Clone)]
pub struct DiffFile {
    pub path: String,
    pub added_lines: usize,
    pub removed_lines: usize,
    pub hunks: Vec<String>,
}

pub fn parse_diff(raw: &str) -> Vec<DiffFile> {
    let mut files: Vec<DiffFile> = Vec::new();
    let mut current: Option<DiffFile> = None;
    let mut current_hunk = String::new();

    for line in raw.lines() {
        if line.starts_with("--- ") || line.starts_with("+++ ") {
            continue;
        }
        if line.starts_with("diff --git ") {
            if let Some(mut f) = current.take() {
                if !current_hunk.is_empty() {
                    f.hunks.push(std::mem::take(&mut current_hunk));
                }
                files.push(f);
            }
            let path = line.split(" b/").nth(1).unwrap_or("unknown").to_string();
            current = Some(DiffFile {
                path,
                added_lines: 0,
                removed_lines: 0,
                hunks: Vec::new(),
            });
            continue;
        }
        if let Some(ref mut f) = current {
            if line.starts_with("@@") {
                if !current_hunk.is_empty() {
                    f.hunks.push(std::mem::take(&mut current_hunk));
                }
            } else if line.starts_with('+') && !line.starts_with("+++") {
                f.added_lines += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                f.removed_lines += 1;
            }
            current_hunk.push_str(line);
            current_hunk.push('\n');
        }
    }

    if let Some(mut f) = current {
        if !current_hunk.is_empty() {
            f.hunks.push(current_hunk);
        }
        files.push(f);
    }

    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_file_diff() {
        let diff = "diff --git a/src/foo.ts b/src/foo.ts\n--- a/src/foo.ts\n+++ b/src/foo.ts\n@@ -1,3 +1,4 @@\n const x = 1;\n-const y = 2;\n+const y = 3;\n+const z = 4;\n";
        let files = parse_diff(diff);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "src/foo.ts");
        assert_eq!(files[0].added_lines, 2);
        assert_eq!(files[0].removed_lines, 1);
    }

    #[test]
    fn test_parse_empty_diff() {
        assert!(parse_diff("").is_empty());
    }

    #[test]
    fn test_parse_multi_file_diff() {
        let diff = "diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-old\n+new\ndiff --git a/b.ts b/b.ts\n--- a/b.ts\n+++ b/b.ts\n@@ -1 +1 @@\n-old2\n+new2\n";
        let files = parse_diff(diff);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "a.ts");
        assert_eq!(files[1].path, "b.ts");
    }

    #[test]
    fn test_parse_diff_new_file_only_additions() {
        // New file: all lines are additions, no removals.
        let diff = "diff --git a/new.ts b/new.ts\n--- /dev/null\n+++ b/new.ts\n@@ -0,0 +1,3 @@\n+line one\n+line two\n+line three\n";
        let files = parse_diff(diff);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].added_lines, 3);
        assert_eq!(files[0].removed_lines, 0);
    }

    #[test]
    fn test_parse_diff_deleted_file_only_removals() {
        // Deleted file: all lines are removals, no additions.
        let diff = "diff --git a/gone.ts b/gone.ts\n--- a/gone.ts\n+++ /dev/null\n@@ -1,2 +0,0 @@\n-remove me\n-and me\n";
        let files = parse_diff(diff);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].added_lines, 0);
        assert_eq!(files[0].removed_lines, 2);
    }

    #[test]
    fn test_parse_diff_deep_nested_path() {
        let diff = "diff --git a/src/utils/helpers/format.ts b/src/utils/helpers/format.ts\n--- a/src/utils/helpers/format.ts\n+++ b/src/utils/helpers/format.ts\n@@ -1 +1 @@\n-old\n+new\n";
        let files = parse_diff(diff);
        assert_eq!(files[0].path, "src/utils/helpers/format.ts");
    }

    #[test]
    fn test_parse_diff_context_lines_not_counted() {
        // Lines starting with a space are context — must not affect add/remove counts.
        let diff = "diff --git a/foo.rs b/foo.rs\n--- a/foo.rs\n+++ b/foo.rs\n@@ -1,4 +1,4 @@\n unchanged line\n another unchanged\n-removed\n+added\n";
        let files = parse_diff(diff);
        assert_eq!(files[0].added_lines, 1);
        assert_eq!(files[0].removed_lines, 1);
    }

    #[test]
    fn test_parse_diff_multiple_hunks_same_file() {
        // Two @@ sections in one file — both hunks collected, counts accumulate.
        let diff = concat!(
            "diff --git a/multi.ts b/multi.ts\n",
            "--- a/multi.ts\n+++ b/multi.ts\n",
            "@@ -1,3 +1,3 @@\n-a\n+b\n",
            "@@ -10,3 +10,3 @@\n-c\n+d\n-e\n",
        );
        let files = parse_diff(diff);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].added_lines, 2);
        assert_eq!(files[0].removed_lines, 3);
        assert_eq!(files[0].hunks.len(), 2);
    }

    #[test]
    fn test_parse_diff_rust_file_extension() {
        let diff = "diff --git a/daemon/src/main.rs b/daemon/src/main.rs\n--- a/daemon/src/main.rs\n+++ b/daemon/src/main.rs\n@@ -1 +1 @@\n-old\n+new\n";
        let files = parse_diff(diff);
        assert_eq!(files[0].path, "daemon/src/main.rs");
    }

    #[test]
    fn test_parse_diff_preserves_file_order() {
        let diff = concat!(
            "diff --git a/z.ts b/z.ts\n@@ -1 +1 @@\n+x\n",
            "diff --git a/a.ts b/a.ts\n@@ -1 +1 @@\n+x\n",
            "diff --git a/m.ts b/m.ts\n@@ -1 +1 @@\n+x\n",
        );
        let files = parse_diff(diff);
        assert_eq!(files[0].path, "z.ts");
        assert_eq!(files[1].path, "a.ts");
        assert_eq!(files[2].path, "m.ts");
    }

    #[test]
    fn test_parse_diff_whitespace_only_diff() {
        // A diff where every changed line is a space (indentation) change.
        // Those lines start with '-' or '+' so they are counted.
        let diff = "diff --git a/indent.ts b/indent.ts\n--- a/indent.ts\n+++ b/indent.ts\n@@ -1 +1 @@\n-    old indent\n+  new indent\n";
        let files = parse_diff(diff);
        assert_eq!(files[0].added_lines, 1);
        assert_eq!(files[0].removed_lines, 1);
    }
}
