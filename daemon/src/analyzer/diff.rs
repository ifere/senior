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
}
