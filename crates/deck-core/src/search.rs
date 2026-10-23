use crate::folder::Folder;

/// Case-insensitive filter with stable ranking:
/// label prefix → label contains → path contains.
pub fn filter<'a>(folders: &'a [Folder], query: &str) -> Vec<&'a Folder> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return folders.iter().collect();
    }

    let mut matches: Vec<(u8, usize, &Folder)> = folders
        .iter()
        .enumerate()
        .filter_map(|(index, folder)| {
            let label = folder.label.to_lowercase();
            let path = folder.path.to_string_lossy().to_lowercase();
            if label.starts_with(&query) {
                Some((0, index, folder))
            } else if label.contains(&query) {
                Some((1, index, folder))
            } else if path.contains(&query) {
                Some((2, index, folder))
            } else {
                None
            }
        })
        .collect();

    matches.sort_by_key(|(rank, index, _)| (*rank, *index));
    matches.into_iter().map(|(_, _, folder)| folder).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn folder(label: &str, path: &str) -> Folder {
        Folder::new(PathBuf::from(path), label, None)
    }

    #[test]
    fn empty_query_keeps_order() {
        let folders = vec![folder("Beta", "/b"), folder("Alpha", "/a")];
        let hits = filter(&folders, "  ");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].label, "Beta");
    }

    #[test]
    fn ranks_prefix_before_substring_before_path() {
        let folders = vec![
            folder("My Website", "/work/site"),
            folder("Website", "/work/website"),
            folder("Archive", "/backup/website-old"),
        ];

        let hits = filter(&folders, "web");
        assert_eq!(
            hits.iter().map(|f| f.label.as_str()).collect::<Vec<_>>(),
            vec!["Website", "My Website", "Archive"]
        );
    }

    #[test]
    fn matching_is_case_insensitive() {
        let folders = vec![folder("ReyyIchiro", "/code/ReyyIchiro")];
        assert_eq!(filter(&folders, "reyy").len(), 1);
        assert_eq!(filter(&folders, "ICHIRO").len(), 1);
        assert!(filter(&folders, "nope").is_empty());
    }
}
